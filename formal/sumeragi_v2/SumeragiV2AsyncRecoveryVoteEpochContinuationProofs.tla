---- MODULE SumeragiV2AsyncRecoveryVoteEpochContinuationProofs ----
EXTENDS SumeragiV2AsyncRecoveryVoteEpochProofs

(***************************************************************************
The imported historical-lock witness begins only after the locked body has
been durably validated.  Keep the earlier certified-body pipeline visible as
a separate, source-neutral obligation.  A scheduled occurrence counts only
for the current consumer epoch; an outstanding request retains one concrete
QcRecord and is matched to the source by its full stable Prepare reference.
Exact wire authentication and exact WAL bytes remain cross-tool obligations;
the reference quotient itself is explicit above.  This invariant is specified
below but intentionally not added to the proved progress bundle: preservation
across every fetch/serve/ingress/store/validate transition remains proof debt.
***************************************************************************)

HistoricalLockedSemanticPrepareAuthority(node, qc, authorityQc) ==
  /\ HistoricalLockedPrepareSource(node, authorityQc)
  /\ authorityQc.context = qc.context
  /\ authorityQc.view = qc.view
  /\ authorityQc.subject = qc.subject

HistoricalLockedCertifiedRequestMatches(node, qc, request) ==
  /\ request.kind = "CertifiedRequest"
  /\ request.source = node
  /\ request.envelope.height = qc.context.height
  /\ request.envelope.view = qc.view
  /\ request.envelope.subject = qc.subject
  /\ \E authorityQc \in prepareQCs:
       /\ HistoricalLockedSemanticPrepareAuthority(
            node, qc, authorityQc)
       /\ request.envelope.recipient
            \in authorityQc.signers \ {node}

HistoricalLockedCertifiedResponseMatches(node, qc, item) ==
  /\ item.kind = "CertifiedResponse"
  /\ item.envelope.recipient = node
  /\ item.envelope.height = qc.context.height
  /\ item.envelope.view = qc.view
  /\ item.envelope.subject = qc.subject
  /\ \E authorityQc \in prepareQCs:
       /\ HistoricalLockedSemanticPrepareAuthority(
            node, qc, authorityQc)
       /\ item.source \in authorityQc.signers

HistoricalLockedBodyPipelineCandidate(node, qc, candidate) ==
  /\ candidate \in AsyncCandidateSet
  /\ candidate.class = "Completion"
  /\ candidate.node = node
  /\ candidate.height = qc.context.height
  /\ candidate.view = qc.view
  /\ candidate.subject = qc.subject
  /\ candidate.kind \in
       {"FetchBody", "RequestCertifiedBody", "FetchCertifiedBody",
        "StoreBody", "ValidateBody"}
  /\ CASE candidate.kind \in {"FetchBody", "RequestCertifiedBody"} ->
            /\ candidate.item = NoAsyncItem
            /\ candidate.evidence \in prepareQCs
            /\ HistoricalLockedSemanticPrepareAuthority(
                 node, qc, candidate.evidence)
       [] candidate.kind = "FetchCertifiedBody" ->
            HistoricalLockedCertifiedResponseMatches(
              node, qc, candidate.item)
       [] OTHER -> TRUE
  /\ CandidateConsumerCurrent(candidate)
  /\ CandidateScheduled(candidate)

HistoricalLockedBodyRecoveryAuthority(node, qc) ==
  /\ asyncRecoveryPhase
       \in {"RestartRequired", "ReplayRequired", "Replaying"}
  /\ asyncRecoveryNode = node
  /\ generation[node] = asyncRecoveryGeneration
  /\ HistoricalLockedPrepareSource(node, qc)

HistoricalLockedCertifiedRequestActive(node, qc) ==
  \E request \in asyncActiveRequests:
    HistoricalLockedCertifiedRequestMatches(node, qc, request)

HistoricalLockedBodyPipelineKindOwned(node, qc, kind) ==
  \E candidate \in AsyncCandidateSet:
    /\ candidate.kind = kind
    /\ HistoricalLockedBodyPipelineCandidate(node, qc, candidate)

HistoricalLockedBodyFetchOwned(node, qc) ==
  HistoricalLockedBodyPipelineKindOwned(node, qc, "FetchBody")

HistoricalLockedBodyRequestOwned(node, qc) ==
  HistoricalLockedBodyPipelineKindOwned(
    node, qc, "RequestCertifiedBody")

HistoricalLockedBodyCertifiedFetchOwned(node, qc) ==
  HistoricalLockedBodyPipelineKindOwned(
    node, qc, "FetchCertifiedBody")

HistoricalLockedBodyStoreOwned(node, qc) ==
  /\ BodyRecord(node, qc.context, qc.view, qc.subject)
       \in availableBodies
  /\ HistoricalLockedBodyPipelineKindOwned(node, qc, "StoreBody")

HistoricalLockedBodyValidateOwned(node, qc) ==
  /\ BodyHeldBy(durableBodies, node, qc.context, qc.view, qc.subject)
  /\ HistoricalLockedBodyPipelineKindOwned(node, qc, "ValidateBody")

HistoricalLockedBodyServeOwned(node, qc) ==
  \E server \in ValidatorIds:
    \E job \in SequenceSet(asyncIoQueues[server]):
      /\ job \in AsyncServeJobSet
      /\ HistoricalLockedCertifiedRequestMatches(
           node, qc, job.candidate.item)

HistoricalLockedBodyResponseInFlight(node, qc) ==
  \E item \in AsyncNetworkItems:
    /\ HistoricalLockedCertifiedResponseMatches(node, qc, item)
    /\ \/ \E packet \in asyncTransport: packet.item = item
       \/ \E source \in AsyncIngressSources:
            item \in SequenceSet(IngressLane(node, source))

HistoricalLockedBodyRestartAuthority(node, qc) ==
  AsyncHistoricalLockRestartAuthority(node, qc)
    \in asyncHistoricalLockRestartAuthorities

(***************************************************************************
Validation is the terminal boundary of the ordinary body-recovery cone, not
an unconditional promise to cast a late historical Commit.  If the exact
lock remains eligible for its old-round Commit, the existing progress-witness
invariant must already own that Commit continuation.  A higher conflicting
Prepare legitimately fences the late Commit, but it does not undo the durable
validation which the later locked-body reproposal obligation consumes.
***************************************************************************)

HistoricalLockedBodyValidated(node, qc) ==
  /\ BodyHeldBy(durableBodies, node, qc.context, qc.view, qc.subject)
  /\ BodyValidatedBy(validatedBodies, node, qc.context, qc.view,
                      generation[node], qc.subject)

HistoricalLockedBodyRecoveryTerminal(node, qc) ==
  /\ HistoricalLockedBodyValidated(node, qc)
  /\ \/ HistoricalLockedCommitRecoveryWitness(node, qc)
     \/ ~HistoricalLockedPrepareForCommit(node, qc)

HistoricalLockedBodyRecoveryStage(node, qc) ==
  \/ HistoricalLockedBodyRecoveryTerminal(node, qc)
  \/ HistoricalLockedCommitRecoveryWitness(node, qc)
  \/ HistoricalLockedBodyRecoveryAuthority(node, qc)
  \/ HistoricalLockedCertifiedRequestActive(node, qc)
  \/ HistoricalLockedBodyRestartAuthority(node, qc)
  \/ HistoricalLockedBodyFetchOwned(node, qc)
  \/ HistoricalLockedBodyRequestOwned(node, qc)
  \/ HistoricalLockedBodyCertifiedFetchOwned(node, qc)
  \/ HistoricalLockedBodyStoreOwned(node, qc)
  \/ HistoricalLockedBodyValidateOwned(node, qc)

HistoricalLockedBodyRecoveryStageInvariant ==
  \A node \in AsyncCurrentResponsiveVoters, qc \in prepareQCs:
    HistoricalLockedPrepareSource(node, qc)
      => HistoricalLockedBodyRecoveryStage(node, qc)

HistoricalLockedBodyRecoveryProperty(specification) ==
  specification => []HistoricalLockedBodyRecoveryStageInvariant

THEOREM HistoricalLockRestartAuthorityEstablishesRecoveryStage ==
  \A node \in ValidatorIds, qc \in prepareQCs:
    HistoricalLockedBodyRestartAuthority(node, qc)
      => HistoricalLockedBodyRecoveryStage(node, qc)
BY DEF HistoricalLockedBodyRecoveryStage

THEOREM HistoricalLockRestartAuthorityRetirementRequiresExactFetch ==
  \A authority \in asyncHistoricalLockRestartAuthorities:
    /\ AsyncHistoricalLockRestartAuthorityTransition
    /\ HistoricalLockRestartAuthoritySourceAfter(authority)
    /\ authority \notin asyncHistoricalLockRestartAuthorities'
    => HistoricalLockRestartExactCurrentFetchOwnerAfter(authority)
BY Isa
   DEF AsyncHistoricalLockRestartAuthorityTransition,
       ResponsiveCrashRecoveryRegistration,
       HistoricalLockRestartAuthoritySourceAfter,
       HistoricalLockRestartExactCurrentFetchOwnerAfter

THEOREM HistoricalLockRestartAuthoritySurvivesGenerationAndReplayReset ==
  \A authority \in asyncHistoricalLockRestartAuthorities:
    /\ AsyncHistoricalLockRestartAuthorityTransition
    /\ HistoricalLockRestartAuthoritySourceAfter(authority)
    /\ ~HistoricalLockRestartExactCurrentFetchOwnerAfter(authority)
    => authority \in asyncHistoricalLockRestartAuthorities'
BY Isa
   DEF AsyncHistoricalLockRestartAuthorityTransition,
       ResponsiveCrashRecoveryRegistration,
       HistoricalLockRestartAuthoritySourceAfter,
       HistoricalLockRestartExactCurrentFetchOwnerAfter

THEOREM ResponsiveCrashRegistersExactHistoricalLockProjection ==
  \A node \in ValidatorIds, qc \in prepareQCs:
    /\ PreGstResponsiveCrash(node)
    /\ HistoricalLockedPrepareSource(node, qc)
    /\ AsyncNext
    => AsyncHistoricalLockRestartAuthority(node, qc)
         \in asyncHistoricalLockRestartAuthorities'
BY Isa
   DEF AsyncNext, AsyncHistoricalLockRestartAuthorityTransition,
       ResponsiveCrashRecoveryRegistration,
       ResponsiveCrashHistoricalLockRestartAuthorities,
       HistoricalLockRestartAuthoritySourceKernel,
       HistoricalLockedPrepareSource,
       HistoricalLockedPrepareRecoveryProvenance,
       InstalledTcSelectsPrepareFor, ExactLockedCommitIntents,
       NoDecisionForNode, PreGstResponsiveCrash

(***************************************************************************
Action-by-action ownership handoffs for the ordinary historical locked-body
cone.  `HistoricalLockedBodyRuntimeExecutes` names only a successful removal
from one of the two serialized reducer carriers; a blocked command remains in
its exact runtime/deferred owner and is handled by the preservation lemmas
below.  The request and response selectors name the actual fair-ingress item,
and Serve ownership names the remote signer's queue rather than the requesting
validator's queue.
***************************************************************************)

HistoricalLockedBodySourceRetired(node, qc) ==
  ~HistoricalLockedPrepareSource(node, qc)

HistoricalLockedBodyRuntimeExecutes(candidate) ==
  /\ [AsyncNext]_AsyncAllVars
  /\ \/ /\ candidate = NextNodeCommand(candidate.node)
           /\ FifoRuntimeStep(candidate.node)
           /\ CommandDispatchable(candidate)
     \/ /\ DeferredQueueNonempty(candidate.node)
           /\ candidate = NextDeferredCommand(candidate.node)
           /\ DeferredDrainStep(candidate.node)
           /\ DeferredHandoffAllowsExecution(candidate.node, candidate)

HistoricalLockedCertifiedRequestSelected(server, node, qc) ==
  /\ asyncIngressReady[server] # <<>>
  /\ DrainableIngressIndices(server) # {}
  /\ HistoricalLockedCertifiedRequestMatches(
       node, qc,
       SelectedIngressItemAt(
         server, FirstDrainableIngressIndex(server)))

HistoricalLockedCertifiedResponseSelected(node, qc) ==
  /\ asyncIngressReady[node] # <<>>
  /\ DrainableIngressIndices(node) # {}
  /\ HistoricalLockedCertifiedResponseMatches(
       node, qc,
       SelectedIngressItemAt(
         node, FirstDrainableIngressIndex(node)))

HistoricalLockedBodyServeHeadOwned(server, node, qc) ==
  /\ AsyncIoQueueDepth(server) > 0
  /\ Head(asyncIoQueues[server]) \in AsyncServeJobSet
  /\ HistoricalLockedCertifiedRequestMatches(
       node, qc, Head(asyncIoQueues[server]).candidate.item)

THEOREM HistoricalLockedFetchExecutionHandsOff ==
  \A node \in ValidatorIds, qc \in prepareQCs,
     candidate \in AsyncCandidateSet:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalLockedPrepareSource(node, qc)
    /\ candidate.kind = "FetchBody"
    /\ HistoricalLockedBodyPipelineCandidate(node, qc, candidate)
    /\ HistoricalLockedBodyRuntimeExecutes(candidate)
    => \/ HistoricalLockedBodySourceRetired(node, qc)'
       \/ HistoricalLockedCertifiedRequestActive(node, qc)'
       \/ HistoricalLockedBodyValidateOwned(node, qc)'
       \/ HistoricalLockedBodyRecoveryTerminal(node, qc)'
BY IsaT(180)
   DEF HistoricalLockedBodyRuntimeExecutes,
       HistoricalLockedBodyPipelineCandidate,
       HistoricalLockedSemanticPrepareAuthority,
       HistoricalLockedBodySourceRetired,
       HistoricalLockedCertifiedRequestActive,
       HistoricalLockedCertifiedRequestMatches,
       HistoricalLockedBodyValidateOwned,
       HistoricalLockedBodyPipelineKindOwned,
       HistoricalLockedBodyRecoveryTerminal,
       HistoricalLockedBodyValidated,
       ExecuteDecisionFetch, PublishCertifiedRequests,
       CertifiedRequestOutbox, CommandSuccessors,
       FreshCommandSuccessors, FreshCandidateSequence,
       AppendCausalSuccessors, FifoRuntimeStep, DeferredDrainStep,
       HistoricalLockedPrepareSource, CertifiedBodyRecoveryAuthority,
       CandidateScheduled, CandidateConsumerCurrent,
       AsyncAllVars

THEOREM HistoricalLockedRequestExecutionHandsOff ==
  \A node \in ValidatorIds, qc \in prepareQCs,
     candidate \in AsyncCandidateSet:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalLockedPrepareSource(node, qc)
    /\ candidate.kind = "RequestCertifiedBody"
    /\ HistoricalLockedBodyPipelineCandidate(node, qc, candidate)
    /\ HistoricalLockedBodyRuntimeExecutes(candidate)
    => \/ HistoricalLockedBodySourceRetired(node, qc)'
       \/ HistoricalLockedCertifiedRequestActive(node, qc)'
       \/ HistoricalLockedBodyRecoveryTerminal(node, qc)'
BY IsaT(180)
   DEF HistoricalLockedBodyRuntimeExecutes,
       HistoricalLockedBodyPipelineCandidate,
       HistoricalLockedSemanticPrepareAuthority,
       HistoricalLockedBodySourceRetired,
       HistoricalLockedCertifiedRequestActive,
       HistoricalLockedCertifiedRequestMatches,
       HistoricalLockedBodyRecoveryTerminal,
       HistoricalLockedBodyValidated,
       ExecuteRequestCertifiedBody, PublishCertifiedRequests,
       CertifiedRequestOutbox, FifoRuntimeStep, DeferredDrainStep,
       HistoricalLockedPrepareSource, CertifiedBodyRecoveryAuthority,
       CandidateScheduled, CandidateConsumerCurrent,
       AsyncAllVars

THEOREM HistoricalLockedCertifiedFetchExecutionHandsOff ==
  \A node \in ValidatorIds, qc \in prepareQCs,
     candidate \in AsyncCandidateSet:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalLockedPrepareSource(node, qc)
    /\ candidate.kind = "FetchCertifiedBody"
    /\ HistoricalLockedBodyPipelineCandidate(node, qc, candidate)
    /\ HistoricalLockedBodyRuntimeExecutes(candidate)
    => \/ HistoricalLockedBodySourceRetired(node, qc)'
       \/ HistoricalLockedBodyStoreOwned(node, qc)'
       \/ HistoricalLockedBodyRecoveryTerminal(node, qc)'
BY IsaT(180)
   DEF HistoricalLockedBodyRuntimeExecutes,
       HistoricalLockedBodyPipelineCandidate,
       HistoricalLockedCertifiedResponseMatches,
       HistoricalLockedSemanticPrepareAuthority,
       HistoricalLockedBodySourceRetired,
       HistoricalLockedBodyStoreOwned,
       HistoricalLockedBodyPipelineKindOwned,
       HistoricalLockedBodyRecoveryTerminal,
       HistoricalLockedBodyValidated,
       ExecuteRegularCommand, RegularCoreCommand,
       FetchCertifiedBody, CommandSuccessors,
       FreshCommandSuccessors, FreshCandidateSequence,
       AppendCausalSuccessors, FifoRuntimeStep, DeferredDrainStep,
       HistoricalLockedPrepareSource, CandidateScheduled,
       CandidateConsumerCurrent, AsyncAllVars

THEOREM HistoricalLockedStoreExecutionHandsOff ==
  \A node \in ValidatorIds, qc \in prepareQCs,
     candidate \in AsyncCandidateSet:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalLockedPrepareSource(node, qc)
    /\ candidate.kind = "StoreBody"
    /\ HistoricalLockedBodyPipelineCandidate(node, qc, candidate)
    /\ HistoricalLockedBodyRuntimeExecutes(candidate)
    => \/ HistoricalLockedBodySourceRetired(node, qc)'
       \/ HistoricalLockedBodyValidateOwned(node, qc)'
       \/ HistoricalLockedBodyRecoveryTerminal(node, qc)'
BY IsaT(180)
   DEF HistoricalLockedBodyRuntimeExecutes,
       HistoricalLockedBodyPipelineCandidate,
       HistoricalLockedBodySourceRetired,
       HistoricalLockedBodyValidateOwned,
       HistoricalLockedBodyPipelineKindOwned,
       HistoricalLockedBodyRecoveryTerminal,
       HistoricalLockedBodyValidated,
       ExecuteRegularCommand, RegularCoreCommand, StoreBody,
       CommandSuccessors, FreshCommandSuccessors,
       FreshCandidateSequence, AppendCausalSuccessors,
       FifoRuntimeStep, DeferredDrainStep,
       HistoricalLockedPrepareSource, CandidateScheduled,
       CandidateConsumerCurrent, AsyncAllVars

THEOREM HistoricalLockedValidateExecutionHandsOff ==
  \A node \in ValidatorIds, qc \in prepareQCs,
     candidate \in AsyncCandidateSet:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalLockedPrepareSource(node, qc)
    /\ candidate.kind = "ValidateBody"
    /\ HistoricalLockedBodyPipelineCandidate(node, qc, candidate)
    /\ HistoricalLockedBodyRuntimeExecutes(candidate)
    => \/ HistoricalLockedBodySourceRetired(node, qc)'
       \/ HistoricalLockedBodyRecoveryTerminal(node, qc)'
       \/ HistoricalLockedCommitRecoveryWitness(node, qc)'
BY IsaT(180)
   DEF HistoricalLockedBodyRuntimeExecutes,
       HistoricalLockedBodyPipelineCandidate,
       HistoricalLockedBodySourceRetired,
       HistoricalLockedBodyRecoveryTerminal,
       HistoricalLockedBodyValidated,
       HistoricalLockedCommitRecoveryWitness,
       ExecuteRegularCommand, RegularCoreCommand, ValidateLockedBody,
       CommandSuccessors, FreshCommandSuccessors,
       FreshCandidateSequence, AppendCausalSuccessors,
       FifoRuntimeStep, DeferredDrainStep,
       HistoricalLockedPrepareSource,
       HistoricalLockedPrepareForCommit,
       CandidateScheduled, CandidateConsumerCurrent,
       AsyncAllVars

THEOREM HistoricalLockedRequestIngressHandsOffToRemoteServe ==
  \A server, node \in ValidatorIds, qc \in prepareQCs:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalLockedPrepareSource(node, qc)
    /\ HistoricalLockedCertifiedRequestActive(node, qc)
    /\ HistoricalLockedCertifiedRequestSelected(server, node, qc)
    /\ [AsyncNext]_AsyncAllVars
    /\ IngressDrainStep(server)
    => /\ HistoricalLockedCertifiedRequestActive(node, qc)'
       /\ HistoricalLockedBodyServeOwned(node, qc)'
BY IsaT(180)
   DEF HistoricalLockedCertifiedRequestSelected,
       HistoricalLockedCertifiedRequestActive,
       HistoricalLockedCertifiedRequestMatches,
       HistoricalLockedBodyServeOwned,
       IngressDrainStep, DrainFairIngressSelected,
       CertifiedRequestAuthorized, AsyncIoCertifiedServeJob,
       CandidateConsumerCurrent, SequenceSet,
       AsyncAllVars

THEOREM HistoricalLockedServeExecutionPublishesResponse ==
  \A server, node \in ValidatorIds, qc \in prepareQCs:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalLockedPrepareSource(node, qc)
    /\ HistoricalLockedCertifiedRequestActive(node, qc)
    /\ HistoricalLockedBodyServeHeadOwned(server, node, qc)
    /\ CertifiedServeCanRespond(
         server, Head(asyncIoQueues[server]).candidate.item)
    /\ [AsyncNext]_AsyncAllVars
    /\ ServiceIoWorkerWork(server)
    => /\ HistoricalLockedCertifiedRequestActive(node, qc)'
       /\ HistoricalLockedBodyResponseInFlight(node, qc)'
BY IsaT(180)
   DEF HistoricalLockedBodyServeHeadOwned,
       HistoricalLockedCertifiedRequestActive,
       HistoricalLockedCertifiedRequestMatches,
       HistoricalLockedBodyResponseInFlight,
       HistoricalLockedCertifiedResponseMatches,
       HistoricalLockedSemanticPrepareAuthority,
       ServiceIoWorkerWork, CertifiedServeCanRespond,
       CertifiedResponseItem, PublishEphemeralItems,
       PacketsForItems, AsyncAllVars

THEOREM HistoricalLockedResponseIngressHandsOffToCertifiedFetch ==
  \A node \in ValidatorIds, qc \in prepareQCs:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalLockedPrepareSource(node, qc)
    /\ HistoricalLockedCertifiedRequestActive(node, qc)
    /\ HistoricalLockedCertifiedResponseSelected(node, qc)
    /\ [AsyncNext]_AsyncAllVars
    /\ IngressDrainStep(node)
    => \/ HistoricalLockedBodySourceRetired(node, qc)'
       \/ HistoricalLockedBodyCertifiedFetchOwned(node, qc)'
       \/ HistoricalLockedBodyRecoveryTerminal(node, qc)'
BY IsaT(180)
   DEF HistoricalLockedCertifiedResponseSelected,
       HistoricalLockedCertifiedRequestActive,
       HistoricalLockedCertifiedRequestMatches,
       HistoricalLockedCertifiedResponseMatches,
       HistoricalLockedSemanticPrepareAuthority,
       HistoricalLockedBodySourceRetired,
       HistoricalLockedBodyCertifiedFetchOwned,
       HistoricalLockedBodyPipelineKindOwned,
       HistoricalLockedBodyPipelineCandidate,
       HistoricalLockedBodyRecoveryTerminal,
       HistoricalLockedBodyValidated,
       IngressDrainStep, DrainFairIngressSelected,
       CertifiedResponseAuthorized, MatchingCertifiedRequests,
       CertifiedResponseCandidate, CandidateScheduled,
       CandidateConsumerCurrent, AsyncAllVars

THEOREM HistoricalLockedPrepareSourceRetiresOnlyLegitimately ==
  \A node \in ValidatorIds, qc \in prepareQCs:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalLockedPrepareSource(node, qc)
    /\ [AsyncNext]_AsyncAllVars
    /\ HistoricalLockedBodySourceRetired(node, qc)'
    => \/ NodeHasDecision(node)'
       \/ lockRank[node]' > qc.view
       \/ lockSubject[node]' # qc.subject
BY IsaT(180)
   DEF HistoricalLockedBodySourceRetired,
       HistoricalLockedPrepareSource,
       HistoricalLockedPrepareRecoveryProvenance,
       InstalledTcSelectsPrepareFor, ExactLockedCommitIntents,
       NoDecisionForNode, AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep, AsyncAllVars

(***************************************************************************
Preservation closes two different proof cases.  An existing durable source
must retain its current owner or take one of the concrete handoffs above.  A
new source can arise only when the reducer durably installs the selected TC
lock (which atomically appends its semantic FetchBody owner) or persists the
old-round Commit intent (which is already terminal).  Keeping those cases
separate prevents a proof from assuming the source in the pre-state and then
vacuously ignoring the install transition which creates it.
***************************************************************************)

THEOREM HistoricalLockedPersistInstallEstablishesSemanticFetch ==
  \A node \in ValidatorIds, qc \in prepareQCs,
     command \in AsyncCandidateSet:
    /\ AsyncStrongTypeInvariant
    /\ command.kind = "PersistInstallTC"
    /\ command.node = node
    /\ HistoricalLockedBodyRuntimeExecutes(command)
    /\ HistoricalLockedPrepareSource(node, qc)'
    => \/ HistoricalLockedBodyFetchOwned(node, qc)'
       \/ HistoricalLockedCommitRecoveryWitness(node, qc)'
       \/ HistoricalLockedBodyRecoveryTerminal(node, qc)'
BY IsaT(240)
   DEF HistoricalLockedBodyRuntimeExecutes,
       HistoricalLockedBodyFetchOwned,
       HistoricalLockedBodyPipelineKindOwned,
       HistoricalLockedBodyPipelineCandidate,
       HistoricalLockedSemanticPrepareAuthority,
       HistoricalLockedCommitRecoveryWitness,
       HistoricalLockedBodyRecoveryTerminal,
       HistoricalLockedBodyValidated,
       ExecutePersistInstall, PersistInstallTC,
       InstallResultingLockedPrepareQCs,
       InstallLockedFetchSuccessor,
       InstallLockedFetchSuccessors,
       InstallCommitSignSuccessors, InstallCommandSuccessors,
       CommandSuccessors, FreshCommandSuccessors,
       FreshCandidateSequence, AppendCausalSuccessors,
       CandidateScheduled, CandidateConsumerCurrent,
       HistoricalLockedPrepareSource,
       HistoricalLockedPrepareRecoveryProvenance,
       AsyncAllVars

THEOREM HistoricalLockedPersistCommitEstablishesTerminalWitness ==
  \A node \in ValidatorIds, qc \in prepareQCs,
     command \in AsyncCandidateSet:
    /\ AsyncStrongTypeInvariant
    /\ command.kind = "PersistLockCommit"
    /\ command.node = node
    /\ command.view = qc.view
    /\ command.subject = qc.subject
    /\ HistoricalLockedBodyRuntimeExecutes(command)
    /\ HistoricalLockedPrepareSource(node, qc)'
    => HistoricalLockedCommitRecoveryWitness(node, qc)'
BY IsaT(180)
   DEF HistoricalLockedBodyRuntimeExecutes,
       HistoricalLockedCommitRecoveryWitness,
       ExecuteRegularCommand, RegularCoreCommand,
       PersistLockCommit, ExactLockedCommitIntents,
       HistoricalLockedPrepareSource,
       CandidateScheduled, AsyncAllVars

THEOREM HistoricalLockedBodyExistingSourceStepPreservation ==
  \A node \in ValidatorIds, qc \in prepareQCs:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalLockedPrepareSource(node, qc)
    /\ HistoricalLockedBodyRecoveryStage(node, qc)
    /\ [AsyncNext]_AsyncAllVars
    => \/ HistoricalLockedBodySourceRetired(node, qc)'
       \/ HistoricalLockedBodyRecoveryStage(node, qc)'
BY HistoricalLockedFetchExecutionHandsOff,
   HistoricalLockedRequestExecutionHandsOff,
   HistoricalLockedCertifiedFetchExecutionHandsOff,
   HistoricalLockedStoreExecutionHandsOff,
   HistoricalLockedValidateExecutionHandsOff,
   HistoricalLockedRequestIngressHandsOffToRemoteServe,
   HistoricalLockedServeExecutionPublishesResponse,
   HistoricalLockedResponseIngressHandsOffToCertifiedFetch,
   HistoricalLockRestartAuthorityRetirementRequiresExactFetch,
   HistoricalLockRestartAuthoritySurvivesGenerationAndReplayReset,
   ResponsiveCrashRegistersExactHistoricalLockProjection,
   IsaT(300)
   DEF HistoricalLockedBodyRecoveryStage,
       HistoricalLockedBodyRecoveryTerminal,
       HistoricalLockedBodyValidated,
       HistoricalLockedCertifiedRequestActive,
       HistoricalLockedBodyRestartAuthority,
       HistoricalLockedBodyFetchOwned,
       HistoricalLockedBodyRequestOwned,
       HistoricalLockedBodyCertifiedFetchOwned,
       HistoricalLockedBodyStoreOwned,
       HistoricalLockedBodyValidateOwned,
       HistoricalLockedBodyPipelineKindOwned,
       HistoricalLockedBodyPipelineCandidate,
       HistoricalLockedBodySourceRetired,
       HistoricalLockedCommitRecoveryWitness,
       AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       RunNode, RunHistoricalRecoveryNode, RunNodeWork,
       SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       SerializedLocalPrecedesServeIngressStep,
       SelectedLocalAdmissionAdvance,
       AsyncServeIngressTargetOnlyTurn, RuntimeStep,
       RunHistoricalServer, OpenHistoricalRecovery,
       DirectCommitCertificateDiscoveryStep,
       DirectHistoricalCommitCertificateDiscoveryStep,
       ServiceIoWorker, ServiceHistoricalRecoveryIoWorker,
       EnqueueIoLocalControl, EnqueueHistoricalRecoveryIoLocalControl,
       AsyncNetworkStep, AdmitIngressPacket, AsyncFaultStep,
       PreGstCrash, PreGstResponsiveCrash, PreGstResponsiveRestart,
       PreGstResponsiveReplay, DriveResponsiveReplayHead,
       FinishResponsiveReplay, RearmResponsiveRecovery,
       AsyncTick, AsyncAllVars

THEOREM HistoricalLockedBodyNewSourceStepEstablishment ==
  \A node \in ValidatorIds, qc \in prepareQCs:
    /\ AsyncStrongTypeInvariant
    /\ ~HistoricalLockedPrepareSource(node, qc)
    /\ [AsyncNext]_AsyncAllVars
    /\ HistoricalLockedPrepareSource(node, qc)'
    => HistoricalLockedBodyRecoveryStage(node, qc)'
BY HistoricalLockedPersistInstallEstablishesSemanticFetch,
   HistoricalLockedPersistCommitEstablishesTerminalWitness,
   IsaT(300)
   DEF HistoricalLockedBodyRecoveryStage,
       HistoricalLockedBodyRecoveryTerminal,
       HistoricalLockedBodyValidated,
       HistoricalLockedCommitRecoveryWitness,
       HistoricalLockedBodyFetchOwned,
       HistoricalLockedBodyPipelineKindOwned,
       HistoricalLockedBodyPipelineCandidate,
       HistoricalLockedSemanticPrepareAuthority,
       HistoricalLockedPrepareSource,
       HistoricalLockedPrepareRecoveryProvenance,
       InstalledTcSelectsPrepareFor, ExactLockedCommitIntents,
       AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       RunNode, RunHistoricalRecoveryNode, RunNodeWork,
       SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       SerializedLocalPrecedesServeIngressStep,
       SelectedLocalAdmissionAdvance,
       AsyncServeIngressTargetOnlyTurn, RuntimeStep,
       FifoRuntimeStep, DeferredDrainStep,
       AsyncAllVars

THEOREM AsyncInitEstablishesHistoricalLockedBodyRecoveryStage ==
  \A initialContext:
    AsyncInitAt(initialContext)
      => HistoricalLockedBodyRecoveryStageInvariant
BY IsaT(120)
   DEF AsyncInitAt, AsyncBaseInitAt,
       HistoricalLockedBodyRecoveryStageInvariant,
       HistoricalLockedPrepareSource,
       HistoricalLockedPrepareRecoveryProvenance,
       InstalledTcSelectsPrepareFor, ExactLockedCommitIntents,
       NoDecisionForNode

THEOREM AsyncBracketPreservesHistoricalLockedBodyRecoveryStage ==
  /\ AsyncStrongTypeInvariant
  /\ HistoricalLockedBodyRecoveryStageInvariant
  /\ [AsyncNext]_AsyncAllVars
  => HistoricalLockedBodyRecoveryStageInvariant'
PROOF
  <1>1. ASSUME AsyncStrongTypeInvariant,
              HistoricalLockedBodyRecoveryStageInvariant,
              [AsyncNext]_AsyncAllVars
         PROVE HistoricalLockedBodyRecoveryStageInvariant'
    <2>1. AsyncCurrentResponsiveVoters'
             = AsyncCurrentResponsiveVoters
      BY <1>1, Isa
         DEF AsyncNext, AsyncCurrentResponsiveVoters,
             CurrentVoters, CurrentEpoch, AsyncAllVars
    <2>2. ASSUME NEW node \in AsyncCurrentResponsiveVoters',
                  NEW qc \in prepareQCs',
                  HistoricalLockedPrepareSource(node, qc)'
           PROVE HistoricalLockedBodyRecoveryStage(node, qc)'
      <3>1. qc \in prepareQCs
        BY <1>1, <2>2, Isa
           DEF AsyncNext, AsyncAllVars
      <3>2. CASE HistoricalLockedPrepareSource(node, qc)
        <4>1. HistoricalLockedBodyRecoveryStage(node, qc)
          BY <1>1, <2>1, <2>2, <3>2, <3>1
             DEF HistoricalLockedBodyRecoveryStageInvariant
        <4>2. \/ HistoricalLockedBodySourceRetired(node, qc)'
               \/ HistoricalLockedBodyRecoveryStage(node, qc)'
          BY <1>1, <3>1, <3>2, <4>1,
             HistoricalLockedBodyExistingSourceStepPreservation
        <4>3. ~HistoricalLockedBodySourceRetired(node, qc)'
          BY <2>2 DEF HistoricalLockedBodySourceRetired
        <4> QED BY <4>2, <4>3
      <3>3. CASE ~HistoricalLockedPrepareSource(node, qc)
        BY <1>1, <2>2, <3>1, <3>3,
           HistoricalLockedBodyNewSourceStepEstablishment
      <3> QED BY <3>2, <3>3
    <2> QED BY <2>2
         DEF HistoricalLockedBodyRecoveryStageInvariant
  <1> QED BY <1>1
THEOREM HistoricalHigherConflictValidationIsTerminal ==
  \A node \in AsyncCurrentResponsiveVoters, qc \in prepareQCs:
    /\ HistoricalLockedPrepareSource(node, qc)
    /\ BodyHeldBy(durableBodies, node, context, qc.view, qc.subject)
    /\ BodyValidatedBy(validatedBodies, node, context, qc.view,
                       generation[node], qc.subject)
    /\ ~NoHigherConflictingPrepareKnown(node, qc)
      => /\ HistoricalLockedBodyRecoveryTerminal(node, qc)
         /\ HistoricalLockedBodyRecoveryStage(node, qc)
BY DEF HistoricalLockedBodyRecoveryTerminal,
       HistoricalLockedBodyRecoveryStage

=============================================================================
