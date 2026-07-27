---- MODULE SumeragiV2AsyncRecoveryVoteEpochProofs ----
EXTENDS SumeragiV2AsyncTimeoutKernelProofs

(***************************************************************************
Strengthened asynchronous induction.  The Core safety proof is reusable
through the refinement boundary, while scheduler state, the concrete
timeout-receipt pool, physical certified-response claim ownership, recovery
type, authority, and execution state, and the serialized Busy readiness kernel
require their own asynchronous preservation arguments.  Keeping these
conjuncts in one invariant makes the final temporal proof an ordinary
Init/Next induction rather than an implicit reachability claim.
***************************************************************************)

AsyncRecoveryExecutionInvariant ==
  asyncRecoveryPhase = "Replaying" =>
    /\ asyncOutstandingTags[asyncRecoveryNode] = {}
    /\ SequenceHasUniqueValues(asyncRecoveryReplayQueue)
    /\ SequenceSet(asyncRecoveryReplayQueue) \cap
         ResponsiveReplayScheduledCandidates(asyncRecoveryNode) = {}

(***************************************************************************
Every restart authority remains the exact node/context/view/subject
projection of a live historical PrepareQC source.  This is not a new durable
write: responsive crash registration projects the pre-existing durable lock,
and the transition removes the projection when that source is decided or
superseded.  The separate handoff predicate below removes it only after the
exact current-generation FetchBody owner is present in scheduler state.
***************************************************************************)

HistoricalLockRestartAuthoritySourceRetentionInvariant ==
  \A authority \in asyncHistoricalLockRestartAuthorities:
    HistoricalLockRestartAuthoritySource(authority)
AsyncGstRecoveryPhaseInvariant ==
  gst =>
    asyncRecoveryPhase
      \notin {"RestartRequired", "ReplayRequired", "Replaying"}

THEOREM AsyncInitEstablishesSerializedBusyKernelInvariant ==
  \A initialContext:
    AsyncInitAt(initialContext) => AsyncSerializedBusyKernelInvariant
BY Isa
   DEF AsyncInitAt, AsyncBaseInitAt, InitAt,
       AsyncSerializedBusyKernelInvariant,
       AsyncBusyReadinessInvariant,
       SerializedBusyOwnershipInvariant, SerializedBusyOwners,
       AllPendingRequests, RequestsUniqueByNode

THEOREM CoreVarsStutterPreservesSerializedBusyKernelInvariant ==
  AsyncSerializedBusyKernelInvariant /\ UNCHANGED vars
    => AsyncSerializedBusyKernelInvariant'
BY Isa
   DEF AsyncSerializedBusyKernelInvariant,
       AsyncBusyReadinessInvariant,
       SerializedBusyOwnershipInvariant, SerializedBusyOwners,
       AllPendingRequests, RequestsUniqueByNode, vars

THEOREM SerializedBusyOwnerNodeSet ==
  RequestNodeSet(SerializedBusyOwners) = PendingNodes \cup SigningNodes
BY Isa
   DEF SerializedBusyOwners, PendingNodes, SigningNodes,
       AllPendingRequests, RequestNodeSet

(***************************************************************************
The direct Core induction below projects exactly the state read by the Busy
kernel.  Ordinary Core actions either frame this tuple, grow only the two
positive evidence carriers, create/convert/remove one serialized owner, or
perform the special InstallTC replacement.
***************************************************************************)

AsyncSerializedBusyKernelVars ==
  <<context, nodeView, durableBodies,
    proposalIntents, prepareIntents, commitIntents, timeoutIntents,
    prepareQCs, lockRank, lockSubject,
    pendingProposal, pendingPrepare, pendingObservePrepare,
    pendingLockCommit, pendingTimeout, pendingInstallTC, pendingDecision,
    signProposals, signVotes, signTimeouts>>

AsyncSerializedBusyKernelGrowthFrameVars ==
  <<context, nodeView,
    proposalIntents, prepareIntents, commitIntents, timeoutIntents,
    lockRank, lockSubject,
    pendingProposal, pendingPrepare, pendingObservePrepare,
    pendingLockCommit, pendingTimeout, pendingInstallTC, pendingDecision,
    signProposals, signVotes, signTimeouts>>

THEOREM SerializedBusyKernelFramePreservesInvariant ==
  /\ AsyncSerializedBusyKernelInvariant
  /\ UNCHANGED AsyncSerializedBusyKernelVars
  => AsyncSerializedBusyKernelInvariant'
BY Isa
   DEF AsyncSerializedBusyKernelVars,
       AsyncSerializedBusyKernelInvariant,
       AsyncBusyReadinessInvariant,
       SerializedBusyOwnershipInvariant, SerializedBusyOwners,
       AllPendingRequests, RequestsUniqueByNode

THEOREM SerializedBusyKernelEvidenceGrowthPreservesInvariant ==
  /\ AsyncSerializedBusyKernelInvariant
  /\ UNCHANGED AsyncSerializedBusyKernelGrowthFrameVars
  /\ durableBodies \subseteq durableBodies'
  /\ prepareQCs \subseteq prepareQCs'
  => AsyncSerializedBusyKernelInvariant'
BY Isa
   DEF AsyncSerializedBusyKernelGrowthFrameVars,
       AsyncSerializedBusyKernelInvariant,
       AsyncBusyReadinessInvariant,
       SerializedBusyOwnershipInvariant, SerializedBusyOwners,
       AllPendingRequests, RequestsUniqueByNode,
       BodyHeldBy, VoteRoundAdmissible, LockedPrepareRound

CoreSerializedBusyExactFrameAction ==
  \/ SetGST
  \/ \E signer \in ValidatorIds, roundView \in Views,
       subject \in Subjects,
       timeoutCertificate \in TimeoutCertificateOptionSet,
       highestPrepare \in PrepareQcOptionSet:
       ByzantineBroadcastProposal(signer, roundView, subject,
                                  timeoutCertificate, highestPrepare)
  \/ \E envelope \in proposalNetwork: DeliverProposal(envelope)
  \/ \E node \in ValidatorIds, proposal \in SeenProposalValues:
       FetchBody(node, proposal)
  \/ \E node \in ValidatorIds, proposal \in SeenProposalValues:
       RebindRetainedBody(node, proposal)
  \/ \E node \in ValidatorIds, proposal \in SeenProposalValues:
       ValidateBody(node, proposal)
  \/ \E node \in ValidatorIds, proposal \in SeenProposalValues:
       RejectBody(node, proposal)
  \/ \E node \in ValidatorIds, qc \in DecisionQcValues:
       ValidateDecidedBody(node, qc)
  \/ \E node \in ValidatorIds, qc \in prepareQCs:
       ValidateLockedBody(node, qc)
  \/ \E signer \in ValidatorIds, roundView \in Views,
       phase \in Phases, subject \in Subjects:
       ByzantineBroadcastVote(signer, roundView, phase, subject)
  \/ \E envelope \in voteNetwork: DeliverVote(envelope)
  \/ \E envelope \in QcEnvelopeSet:
       ImportAuthenticatedCommitCertificate(envelope)
  \/ \E envelope \in qcNetwork: DeliverQC(envelope)
  \/ \E signer \in ValidatorIds, roundView \in Views,
       highestPrepare \in PrepareQcOptionSet:
       ByzantineBroadcastTimeout(signer, roundView, highestPrepare)
  \/ \E envelope \in timeoutNetwork: DeliverTimeout(envelope)
  \/ \E envelope \in tcNetwork: DeliverTC(envelope)
  \/ \E node \in ValidatorIds,
       qc \in DecisionQcValues \cup prepareQCs:
       FetchCertifiedBody(node, qc)
  \/ \E node \in ValidatorIds, roundView \in Views,
       subject \in Subjects:
       AcceptCertifiedResponseCapability(node, roundView, subject)
  \/ \E node \in ValidatorIds, qc \in DecisionQcValues:
       ApplyDecision(node, qc)
  \/ \E node \in ValidatorIds: Restart(node)
  \/ \E envelope \in proposalNetwork: DropProposal(envelope)

CoreSerializedBusyEvidenceGrowthAction ==
  \/ \E node \in ValidatorIds, subject \in Subjects:
       AssembleLocalBody(node, subject)
  \/ \E node \in ValidatorIds, roundView \in Views, subject \in Subjects:
       StoreBody(node, roundView, subject)
  \/ \E node \in ValidatorIds, roundView \in Views, subject \in Subjects:
       FormPrepareQC(node, roundView, subject)

CoreSerializedBusyOwnerCreationAction ==
  \/ \E node \in ValidatorIds, subject \in Subjects:
       BeginLocalProposal(node, subject)
  \/ \E node \in ValidatorIds, proposal \in SeenProposalValues:
       BeginPrepare(node, proposal)
  \/ \E node \in ValidatorIds, qc \in ReceivedQcValues:
       BeginObservePrepare(node, qc)
  \/ \E node \in ValidatorIds, qc \in LockCommitQcValues:
       BeginLockCommit(node, qc)
  \/ \E node \in ValidatorIds, roundView \in Views, subject \in Subjects:
       FormCommitQC(node, roundView, subject)
  \/ \E node \in ValidatorIds, qc \in ReceivedQcValues:
       BeginDecision(node, qc)
  \/ \E node \in ValidatorIds: BeginTimeout(node)
  \/ \E node \in ValidatorIds, roundView \in Views:
       FormTC(node, roundView)
  \/ \E node \in ValidatorIds, tc \in ReceivedTcValues:
       BeginInstallTC(node, tc)
  \/ \E node \in ValidatorIds, proposal \in proposalIntents:
       ResumeProposal(node, proposal)
  \/ \E node \in ValidatorIds, vote \in prepareIntents \cup commitIntents:
       ResumeVote(node, vote)
  \/ \E node \in ValidatorIds, vote \in timeoutIntents:
       ResumeTimeout(node, vote)

CoreSerializedBusyOwnerConversionAction ==
  \/ \E request \in pendingProposal: PersistProposal(request)
  \/ \E request \in pendingPrepare: PersistPrepare(request)
  \/ \E request \in pendingLockCommit: PersistLockCommit(request)
  \/ \E request \in pendingTimeout: PersistTimeout(request)

CoreSerializedBusyOwnerRemovalAction ==
  \/ \E request \in signProposals: CompleteProposalSignature(request)
  \/ \E request \in signVotes: CompleteVoteSignature(request)
  \/ \E request \in pendingObservePrepare:
       PersistObservePrepare(request)
  \/ \E request \in pendingDecision: PersistDecision(request)
  \/ \E request \in signTimeouts: CompleteTimeoutSignature(request)

CoreSerializedBusyCrashAction ==
  \E node \in ValidatorIds: Crash(node)

CoreSerializedBusyInstallAction ==
  \E request \in pendingInstallTC: PersistInstallTC(request)

THEOREM CoreNextSerializedBusyActionClassification ==
  Next
    => \/ CoreSerializedBusyExactFrameAction
       \/ CoreSerializedBusyEvidenceGrowthAction
       \/ CoreSerializedBusyOwnerCreationAction
       \/ CoreSerializedBusyOwnerConversionAction
       \/ CoreSerializedBusyOwnerRemovalAction
       \/ CoreSerializedBusyCrashAction
       \/ CoreSerializedBusyInstallAction
BY DEF Next,
       CoreSerializedBusyExactFrameAction,
       CoreSerializedBusyEvidenceGrowthAction,
       CoreSerializedBusyOwnerCreationAction,
       CoreSerializedBusyOwnerConversionAction,
       CoreSerializedBusyOwnerRemovalAction,
       CoreSerializedBusyCrashAction,
       CoreSerializedBusyInstallAction

THEOREM CoreSerializedBusyExactFrameActionPreservesKernel ==
  /\ AsyncSerializedBusyKernelInvariant
  /\ CoreSerializedBusyExactFrameAction
  => AsyncSerializedBusyKernelInvariant'
BY SerializedBusyKernelFramePreservesInvariant, Isa
   DEF CoreSerializedBusyExactFrameAction,
       AsyncSerializedBusyKernelVars,
       SetGST, ByzantineBroadcastProposal, DeliverProposal,
       FetchBody, RebindRetainedBody, ValidateBody, RejectBody,
       ValidateDecidedBody, ValidateLockedBody,
       ByzantineBroadcastVote, DeliverVote,
       ImportAuthenticatedCommitCertificate, DeliverQC,
       ByzantineBroadcastTimeout, DeliverTimeout, DeliverTC,
       FetchCertifiedBody, AcceptCertifiedResponseCapability,
       InstallCertifiedBodyEffect, ApplyDecision, Restart, DropProposal

THEOREM CoreSerializedBusyEvidenceGrowthActionPreservesKernel ==
  /\ AsyncSerializedBusyKernelInvariant
  /\ CoreSerializedBusyEvidenceGrowthAction
  => AsyncSerializedBusyKernelInvariant'
BY SerializedBusyKernelEvidenceGrowthPreservesInvariant, Isa
   DEF CoreSerializedBusyEvidenceGrowthAction,
       AsyncSerializedBusyKernelGrowthFrameVars,
       AssembleLocalBody, StoreBody, FormPrepareQC

THEOREM NodeIdleExcludesSerializedBusyOwnerNode ==
  \A node:
    NodeIdle(node)
      => node \notin RequestNodeSet(SerializedBusyOwners)
BY SerializedBusyOwnerNodeSet
   DEF NodeIdle

THEOREM AddingFreshSerializedBusyOwnerPreservesOwnership ==
  \A request:
    /\ SerializedBusyOwnershipInvariant
    /\ NodeIdle(request.node)
    /\ SerializedBusyOwners' =
         SerializedBusyOwners \cup {request}
    => SerializedBusyOwnershipInvariant'
PROOF
  <1>1. ASSUME NEW request,
                SerializedBusyOwnershipInvariant,
                NodeIdle(request.node),
                SerializedBusyOwners' =
                  SerializedBusyOwners \cup {request}
         PROVE SerializedBusyOwnershipInvariant'
    <2>1. RequestsUniqueByNode(SerializedBusyOwners)
      BY <1>1 DEF SerializedBusyOwnershipInvariant
    <2>2. request.node
             \notin RequestNodeSet(SerializedBusyOwners)
      BY <1>1, NodeIdleExcludesSerializedBusyOwnerNode
    <2>3. RequestsUniqueByNode(
             SerializedBusyOwners \cup {request})
      BY <2>1, <2>2, NewRequestPreservesNodeUniqueness
    <2> QED BY <1>1, <2>3
         DEF SerializedBusyOwnershipInvariant
  <1> QED BY <1>1

THEOREM AddingReadySerializedBusyOwnerPreservesKernel ==
  \A request:
    /\ AsyncSerializedBusyKernelInvariant
    /\ NodeIdle(request.node)
    /\ SerializedBusyOwners' =
         SerializedBusyOwners \cup {request}
    /\ AsyncBusyReadinessInvariant'
    => AsyncSerializedBusyKernelInvariant'
BY AddingFreshSerializedBusyOwnerPreservesOwnership
   DEF AsyncSerializedBusyKernelInvariant

THEOREM CoreBeginLocalProposalPreservesSerializedBusyKernel ==
  \A node, subject:
    /\ StrongInductiveInvariant
    /\ AsyncSerializedBusyKernelInvariant
    /\ BeginLocalProposal(node, subject)
    => AsyncSerializedBusyKernelInvariant'
BY AddingReadySerializedBusyOwnerPreservesKernel, IsaT(60)
   DEF BeginLocalProposal, LocalProposalFor, ProposalWal,
       AsyncBusyReadinessInvariant,
       SerializedBusyOwners, AllPendingRequests

THEOREM CoreBeginPreparePreservesSerializedBusyKernel ==
  \A node, proposal:
    /\ StrongInductiveInvariant
    /\ AsyncSerializedBusyKernelInvariant
    /\ BeginPrepare(node, proposal)
    => AsyncSerializedBusyKernelInvariant'
BY AddingReadySerializedBusyOwnerPreservesKernel, IsaT(60)
   DEF BeginPrepare, PrepareRequestFor, PrepareVoteFor, PrepareWal,
       AsyncBusyReadinessInvariant,
       SerializedBusyOwners, AllPendingRequests

THEOREM CoreBeginObservePreparePreservesSerializedBusyKernel ==
  \A node, qc:
    /\ StrongInductiveInvariant
    /\ AsyncSerializedBusyKernelInvariant
    /\ BeginObservePrepare(node, qc)
    => AsyncSerializedBusyKernelInvariant'
BY AddingReadySerializedBusyOwnerPreservesKernel, Isa
   DEF BeginObservePrepare, ObservePrepareWal,
       AsyncBusyReadinessInvariant,
       SerializedBusyOwners, AllPendingRequests

THEOREM CoreBeginLockCommitPreservesSerializedBusyKernel ==
  \A node, qc:
    /\ StrongInductiveInvariant
    /\ AsyncSerializedBusyKernelInvariant
    /\ BeginLockCommit(node, qc)
    => AsyncSerializedBusyKernelInvariant'
BY AddingReadySerializedBusyOwnerPreservesKernel, IsaT(120)
   DEF StrongInductiveInvariant, Safety, TypeInvariant,
       BeginLockCommit, LockCommitWal, Vote,
       AsyncBusyReadinessInvariant,
       SerializedBusyOwners, AllPendingRequests,
       RetainedLockedBodyRecord, RetainedLockedBodyRecordSet,
       ValidatorIds, QcRecordSet, ContextRecords, Subjects

THEOREM CoreFormCommitQCPreservesSerializedBusyKernel ==
  \A node, roundView, subject:
    /\ StrongInductiveInvariant
    /\ AsyncSerializedBusyKernelInvariant
    /\ FormCommitQC(node, roundView, subject)
    => AsyncSerializedBusyKernelInvariant'
BY AddingReadySerializedBusyOwnerPreservesKernel, Isa
   DEF FormCommitQC, DecisionWal,
       AsyncBusyReadinessInvariant,
       SerializedBusyOwners, AllPendingRequests

THEOREM CoreBeginDecisionPreservesSerializedBusyKernel ==
  \A node, qc:
    /\ StrongInductiveInvariant
    /\ AsyncSerializedBusyKernelInvariant
    /\ BeginDecision(node, qc)
    => AsyncSerializedBusyKernelInvariant'
BY AddingReadySerializedBusyOwnerPreservesKernel, Isa
   DEF BeginDecision, DecisionWal,
       AsyncBusyReadinessInvariant,
       SerializedBusyOwners, AllPendingRequests

THEOREM CoreBeginTimeoutPreservesSerializedBusyKernel ==
  \A node:
    /\ StrongInductiveInvariant
    /\ AsyncSerializedBusyKernelInvariant
    /\ BeginTimeout(node)
    => AsyncSerializedBusyKernelInvariant'
BY AddingReadySerializedBusyOwnerPreservesKernel, IsaT(60)
   DEF BeginTimeout, TimeoutRequestFor, LocalTimeoutVoteFor,
       TimeoutWal, NodeTimedOut,
       AsyncBusyReadinessInvariant,
       SerializedBusyOwners, AllPendingRequests

THEOREM CoreFormTCPreservesSerializedBusyKernel ==
  \A node, roundView:
    /\ StrongInductiveInvariant
    /\ AsyncSerializedBusyKernelInvariant
    /\ FormTC(node, roundView)
    => AsyncSerializedBusyKernelInvariant'
BY AddingReadySerializedBusyOwnerPreservesKernel, Isa
   DEF FormTC, InstallTcWal,
       AsyncBusyReadinessInvariant,
       SerializedBusyOwners, AllPendingRequests

THEOREM CoreBeginInstallTCPreservesSerializedBusyKernel ==
  \A node, tc:
    /\ StrongInductiveInvariant
    /\ AsyncSerializedBusyKernelInvariant
    /\ BeginInstallTC(node, tc)
    => AsyncSerializedBusyKernelInvariant'
BY AddingReadySerializedBusyOwnerPreservesKernel, Isa
   DEF BeginInstallTC, InstallTcWal,
       AsyncBusyReadinessInvariant,
       SerializedBusyOwners, AllPendingRequests

THEOREM CoreResumeProposalPreservesSerializedBusyKernel ==
  \A node, proposal:
    /\ StrongInductiveInvariant
    /\ AsyncSerializedBusyKernelInvariant
    /\ ResumeProposal(node, proposal)
    => AsyncSerializedBusyKernelInvariant'
BY AddingReadySerializedBusyOwnerPreservesKernel, Isa
   DEF ResumeProposal, ProposalSign,
       AsyncBusyReadinessInvariant,
       SerializedBusyOwners, AllPendingRequests

THEOREM CoreResumeVotePreservesSerializedBusyKernel ==
  \A node, vote:
    /\ StrongInductiveInvariant
    /\ AsyncSerializedBusyKernelInvariant
    /\ ResumeVote(node, vote)
    => AsyncSerializedBusyKernelInvariant'
BY AddingReadySerializedBusyOwnerPreservesKernel, Isa
   DEF ResumeVote, VoteResumeAuthorized, VoteSign,
       VoteRoundAdmissible,
       AsyncBusyReadinessInvariant,
       SerializedBusyOwners, AllPendingRequests

THEOREM CoreResumeTimeoutPreservesSerializedBusyKernel ==
  \A node, vote:
    /\ StrongInductiveInvariant
    /\ AsyncSerializedBusyKernelInvariant
    /\ ResumeTimeout(node, vote)
    => AsyncSerializedBusyKernelInvariant'
BY AddingReadySerializedBusyOwnerPreservesKernel, Isa
   DEF ResumeTimeout, TimeoutSign,
       AsyncBusyReadinessInvariant,
       SerializedBusyOwners, AllPendingRequests

THEOREM CoreSerializedBusyOwnerCreationActionPreservesKernel ==
  /\ StrongInductiveInvariant
  /\ AsyncSerializedBusyKernelInvariant
  /\ CoreSerializedBusyOwnerCreationAction
  => AsyncSerializedBusyKernelInvariant'
BY CoreBeginLocalProposalPreservesSerializedBusyKernel,
   CoreBeginPreparePreservesSerializedBusyKernel,
   CoreBeginObservePreparePreservesSerializedBusyKernel,
   CoreBeginLockCommitPreservesSerializedBusyKernel,
   CoreFormCommitQCPreservesSerializedBusyKernel,
   CoreBeginDecisionPreservesSerializedBusyKernel,
   CoreBeginTimeoutPreservesSerializedBusyKernel,
   CoreFormTCPreservesSerializedBusyKernel,
   CoreBeginInstallTCPreservesSerializedBusyKernel,
   CoreResumeProposalPreservesSerializedBusyKernel,
   CoreResumeVotePreservesSerializedBusyKernel,
   CoreResumeTimeoutPreservesSerializedBusyKernel
   DEF CoreSerializedBusyOwnerCreationAction

THEOREM ReplacingSerializedBusyOwnerAtSameNodePreservesOwnership ==
  \A oldOwner, newOwner:
    /\ SerializedBusyOwnershipInvariant
    /\ oldOwner \in SerializedBusyOwners
    /\ newOwner.node = oldOwner.node
    /\ SerializedBusyOwners' =
         (SerializedBusyOwners \ {oldOwner}) \cup {newOwner}
    => SerializedBusyOwnershipInvariant'
BY SMT
   DEF SerializedBusyOwnershipInvariant, RequestsUniqueByNode

THEOREM ConvertingReadySerializedBusyOwnerPreservesKernel ==
  \A oldOwner, newOwner:
    /\ AsyncSerializedBusyKernelInvariant
    /\ oldOwner \in SerializedBusyOwners
    /\ newOwner.node = oldOwner.node
    /\ SerializedBusyOwners' =
         (SerializedBusyOwners \ {oldOwner}) \cup {newOwner}
    /\ AsyncBusyReadinessInvariant'
    => AsyncSerializedBusyKernelInvariant'
BY ReplacingSerializedBusyOwnerAtSameNodePreservesOwnership
   DEF AsyncSerializedBusyKernelInvariant

THEOREM CorePersistProposalPreservesSerializedBusyKernel ==
  \A request:
    /\ StrongInductiveInvariant
    /\ AsyncSerializedBusyKernelInvariant
    /\ PersistProposal(request)
    => AsyncSerializedBusyKernelInvariant'
BY ConvertingReadySerializedBusyOwnerPreservesKernel, IsaT(120)
   DEF PersistProposal, ProposalSign,
       AsyncSerializedBusyKernelInvariant,
       AsyncBusyReadinessInvariant,
       SerializedBusyOwnershipInvariant, RequestsUniqueByNode,
       SerializedBusyOwners, AllPendingRequests

THEOREM CorePersistPreparePreservesSerializedBusyKernel ==
  \A request:
    /\ StrongInductiveInvariant
    /\ AsyncSerializedBusyKernelInvariant
    /\ PersistPrepare(request)
    => AsyncSerializedBusyKernelInvariant'
BY ConvertingReadySerializedBusyOwnerPreservesKernel, IsaT(180)
   DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
       PendingVoteWritesAuthorized,
       PersistPrepare, VoteSign,
       VoteRoundAdmissible,
       AsyncSerializedBusyKernelInvariant,
       AsyncBusyReadinessInvariant,
       SerializedBusyOwnershipInvariant, RequestsUniqueByNode,
       SerializedBusyOwners, AllPendingRequests

THEOREM CorePersistLockCommitPreservesSerializedBusyKernel ==
  \A request:
    /\ StrongInductiveInvariant
    /\ AsyncSerializedBusyKernelInvariant
    /\ PersistLockCommit(request)
    => AsyncSerializedBusyKernelInvariant'
BY ConvertingReadySerializedBusyOwnerPreservesKernel, IsaT(240)
   DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
       PendingVoteWritesAuthorized,
       PersistLockCommit, VoteSign,
       VoteRoundAdmissible, LockedPrepareRound,
       AsyncSerializedBusyKernelInvariant,
       AsyncBusyReadinessInvariant,
       SerializedBusyOwnershipInvariant, RequestsUniqueByNode,
       SerializedBusyOwners, AllPendingRequests

THEOREM CorePersistTimeoutPreservesSerializedBusyKernel ==
  \A request:
    /\ StrongInductiveInvariant
    /\ AsyncSerializedBusyKernelInvariant
    /\ PersistTimeout(request)
    => AsyncSerializedBusyKernelInvariant'
BY ConvertingReadySerializedBusyOwnerPreservesKernel, IsaT(180)
   DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
       PendingVoteWritesAuthorized,
       PersistTimeout, TimeoutSign,
       AsyncSerializedBusyKernelInvariant,
       AsyncBusyReadinessInvariant,
       SerializedBusyOwnershipInvariant, RequestsUniqueByNode,
       SerializedBusyOwners, AllPendingRequests

THEOREM CoreSerializedBusyOwnerConversionActionPreservesKernel ==
  /\ StrongInductiveInvariant
  /\ AsyncSerializedBusyKernelInvariant
  /\ CoreSerializedBusyOwnerConversionAction
  => AsyncSerializedBusyKernelInvariant'
BY CorePersistProposalPreservesSerializedBusyKernel,
   CorePersistPreparePreservesSerializedBusyKernel,
   CorePersistLockCommitPreservesSerializedBusyKernel,
   CorePersistTimeoutPreservesSerializedBusyKernel
   DEF CoreSerializedBusyOwnerConversionAction

AsyncBusyReadinessGuardVars ==
  <<context, nodeView, durableBodies,
    proposalIntents, prepareIntents, commitIntents, timeoutIntents,
    prepareQCs, lockRank, lockSubject>>

THEOREM RemovingSerializedBusyOwnersPreservesKernel ==
  /\ AsyncSerializedBusyKernelInvariant
  /\ UNCHANGED AsyncBusyReadinessGuardVars
  /\ pendingProposal' \subseteq pendingProposal
  /\ pendingPrepare' \subseteq pendingPrepare
  /\ pendingObservePrepare' \subseteq pendingObservePrepare
  /\ pendingLockCommit' \subseteq pendingLockCommit
  /\ pendingTimeout' \subseteq pendingTimeout
  /\ pendingInstallTC' \subseteq pendingInstallTC
  /\ pendingDecision' \subseteq pendingDecision
  /\ signProposals' \subseteq signProposals
  /\ signVotes' \subseteq signVotes
  /\ signTimeouts' \subseteq signTimeouts
  => AsyncSerializedBusyKernelInvariant'
PROOF
  <1>1. ASSUME AsyncSerializedBusyKernelInvariant,
              UNCHANGED AsyncBusyReadinessGuardVars,
              pendingProposal' \subseteq pendingProposal,
              pendingPrepare' \subseteq pendingPrepare,
              pendingObservePrepare' \subseteq pendingObservePrepare,
              pendingLockCommit' \subseteq pendingLockCommit,
              pendingTimeout' \subseteq pendingTimeout,
              pendingInstallTC' \subseteq pendingInstallTC,
              pendingDecision' \subseteq pendingDecision,
              signProposals' \subseteq signProposals,
              signVotes' \subseteq signVotes,
              signTimeouts' \subseteq signTimeouts
         PROVE AsyncSerializedBusyKernelInvariant'
    <2>1. SerializedBusyOwners' \subseteq SerializedBusyOwners
      BY <1>1, Isa
         DEF SerializedBusyOwners, AllPendingRequests
    <2>2. SerializedBusyOwnershipInvariant'
      BY <1>1, <2>1, RemovingRequestsPreservesNodeUniqueness
         DEF AsyncSerializedBusyKernelInvariant,
             SerializedBusyOwnershipInvariant
    <2>3. AsyncBusyReadinessInvariant'
      BY <1>1, Isa
         DEF AsyncSerializedBusyKernelInvariant,
             AsyncBusyReadinessInvariant,
             AsyncBusyReadinessGuardVars,
             VoteRoundAdmissible, LockedPrepareRound
    <2> QED BY <2>2, <2>3
         DEF AsyncSerializedBusyKernelInvariant
  <1> QED BY <1>1

THEOREM CoreSerializedBusyOwnerRemovalActionPreservesKernel ==
  /\ AsyncSerializedBusyKernelInvariant
  /\ CoreSerializedBusyOwnerRemovalAction
  => AsyncSerializedBusyKernelInvariant'
BY RemovingSerializedBusyOwnersPreservesKernel, Isa
   DEF CoreSerializedBusyOwnerRemovalAction,
       AsyncBusyReadinessGuardVars,
       CompleteProposalSignature, CompleteVoteSignature,
       PersistObservePrepare, PersistDecision,
       CompleteTimeoutSignature

THEOREM CoreSerializedBusyCrashActionPreservesKernel ==
  /\ AsyncSerializedBusyKernelInvariant
  /\ CoreSerializedBusyCrashAction
  => AsyncSerializedBusyKernelInvariant'
BY RemovingSerializedBusyOwnersPreservesKernel, Isa
   DEF CoreSerializedBusyCrashAction,
       AsyncBusyReadinessGuardVars, Crash

THEOREM ActiveLockedCommitSignRequestsAfterInstallIsCanonical ==
  \A node, tc:
    \/ ActiveLockedCommitSignRequestsAfterInstall(node, tc) = {}
    \/ ActiveLockedCommitSignRequestsAfterInstall(node, tc) =
         {VoteSign(
            node,
            Vote(context, ResultingInstallLockRank(node, tc),
                 "Commit", ResultingInstallLockSubject(node, tc), node))}
BY SMT
   DEF ActiveLockedCommitSignRequestsAfterInstall,
       ExactLockedCommitIntents

THEOREM ActiveLockedCommitSignRequestsAfterInstallIsUniqueAtNode ==
  \A node, tc:
    /\ RequestsUniqueByNode(
         ActiveLockedCommitSignRequestsAfterInstall(node, tc))
    /\ \A request \in
             ActiveLockedCommitSignRequestsAfterInstall(node, tc):
         request.node = node
BY ActiveLockedCommitSignRequestsAfterInstallIsCanonical, SMT
   DEF RequestsUniqueByNode, VoteSign

THEOREM ReplacingSerializedBusyOwnerBySameNodeSetPreservesOwnership ==
  \A oldOwner, newOwners:
    /\ SerializedBusyOwnershipInvariant
    /\ oldOwner \in SerializedBusyOwners
    /\ RequestsUniqueByNode(newOwners)
    /\ \A newOwner \in newOwners:
         newOwner.node = oldOwner.node
    /\ SerializedBusyOwners' =
         (SerializedBusyOwners \ {oldOwner}) \cup newOwners
    => SerializedBusyOwnershipInvariant'
BY SMT
   DEF SerializedBusyOwnershipInvariant, RequestsUniqueByNode

THEOREM PendingInstallTCResultingLockIsCertified ==
  \A request:
    /\ StrongInductiveInvariant
    /\ request \in pendingInstallTC
    => LET node == request.node
           tc == request.tc
           resultingRank == ResultingInstallLockRank(node, tc)
           resultingSubject == ResultingInstallLockSubject(node, tc)
       IN /\ (resultingRank = NoRank
                => resultingSubject = NoSubject)
          /\ (resultingRank # NoRank
                => \E qc \in prepareQCs:
                     /\ qc.context = context
                     /\ qc.view = resultingRank
                     /\ qc.phase = "Prepare"
                     /\ qc.subject = resultingSubject)
PROOF
  <1>1. ASSUME NEW request,
                StrongInductiveInvariant,
                request \in pendingInstallTC
         PROVE LET node == request.node
                   tc == request.tc
                   resultingRank ==
                     ResultingInstallLockRank(node, tc)
                   resultingSubject ==
                     ResultingInstallLockSubject(node, tc)
               IN /\ (resultingRank = NoRank
                        => resultingSubject = NoSubject)
                  /\ (resultingRank # NoRank
                        => \E qc \in prepareQCs:
                             /\ qc.context = context
                             /\ qc.view = resultingRank
                             /\ qc.phase = "Prepare"
                             /\ qc.subject = resultingSubject)
    <2> DEFINE Node == request.node
    <2> DEFINE Certificate == request.tc
    <2> DEFINE SelectedRank == TcHighRank(Certificate)
    <2> DEFINE SelectedSubject == TcHighSubject(Certificate)
    <2>1. /\ ModelConfiguration
           /\ Node \in ValidatorIds
           /\ Certificate \in formedTCs
           /\ Certificate.context = context
           /\ TCValid(Certificate)
           /\ Certificate.votes # {}
           /\ lockRank[Node] \in Ranks
      BY <1>1, Isa
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             ReducerProvenanceInvariant,
             PendingCertificateWritesAuthorized,
             InstallTcWalSet, Node, Certificate
    <2>2. HighestTimeoutVote(Certificate.votes)
             \in Certificate.votes
      BY <2>1, ValidTimeoutCertificateSelectsMember
    <2>3. HighRefValid(SelectedRank, SelectedSubject)
      BY <2>1, <2>2
         DEF TCValid, AuthenticatedHighRef,
             TcHighRank, TcHighSubject,
             SelectedRank, SelectedSubject, Certificate
    <2>4. /\ CertificatePhasesCorrect
           /\ HighestAndLockAreCertified
      BY <1>1
         DEF StrongInductiveInvariant,
             ReducerProvenanceInvariant, LineageInvariant
    <2>5. \/ /\ SelectedRank = NoRank
                /\ SelectedSubject = NoSubject
           \/ /\ SelectedRank \in Views
                /\ SelectedSubject \in Subjects
                /\ \E qc \in prepareQCs:
                     /\ qc.context = context
                     /\ qc.view = SelectedRank
                     /\ qc.phase = "Prepare"
                     /\ qc.subject = SelectedSubject
      BY <2>3, <2>4, Isa DEF HighRefValid, CertificatePhasesCorrect
    <2>6. /\ (lockRank[Node] = NoRank
                => lockSubject[Node] = NoSubject)
           /\ (lockRank[Node] # NoRank
                => \E qc \in prepareQCs:
                     /\ qc.context = context
                     /\ qc.view = lockRank[Node]
                     /\ qc.phase = "Prepare"
                     /\ qc.subject = lockSubject[Node])
      BY <2>1, <2>4, Isa
         DEF HighestAndLockAreCertified, CertificatePhasesCorrect
    <2>7. /\ SelectedRank \in Ranks
           /\ SelectedRank \in Int
           /\ lockRank[Node] \in Int
      BY <2>1, <2>5, SMT
         DEF Ranks, Views, NoRank
    <2>8. CASE SelectedRank > lockRank[Node]
      <3>1. /\ ResultingInstallLockRank(Node, Certificate) =
                    SelectedRank
             /\ ResultingInstallLockSubject(Node, Certificate) =
                    SelectedSubject
        BY <2>8
           DEF ResultingInstallLockRank,
               ResultingInstallLockSubject,
               SelectedRank, SelectedSubject, Certificate
      <3>2. SelectedRank # NoRank
        BY <2>1, <2>5, <2>7, <2>8, SMT
           DEF Ranks, Views, NoRank
      <3> QED BY <2>5, <3>1, <3>2, Isa
    <2>9. CASE SelectedRank <= lockRank[Node]
      <3>1. /\ ResultingInstallLockRank(Node, Certificate) =
                    lockRank[Node]
             /\ ResultingInstallLockSubject(Node, Certificate) =
                    lockSubject[Node]
        BY <2>9
           DEF ResultingInstallLockRank,
               ResultingInstallLockSubject,
               SelectedRank, SelectedSubject, Certificate
      <3> QED BY <2>6, <3>1, Isa
    <2>10. SelectedRank > lockRank[Node]
               \/ SelectedRank <= lockRank[Node]
      BY <2>7, SMT
    <2> QED BY <2>8, <2>9, <2>10 DEF Node, Certificate
  <1> QED BY <1>1

THEOREM PersistInstallActiveSignRequestsAreReady ==
  \A request:
    /\ StrongInductiveInvariant
    /\ PersistInstallTC(request)
    => \A signRequest \in
             ActiveLockedCommitSignRequestsAfterInstall(
               request.node, request.tc):
         /\ signRequest.vote.signer = signRequest.node
         /\ signRequest.vote \in commitIntents'
         /\ VoteRoundAdmissible(
              signRequest.node, signRequest.vote)'
PROOF
  <1>1. ASSUME NEW request,
                StrongInductiveInvariant,
                PersistInstallTC(request)
         PROVE \A signRequest \in
                   ActiveLockedCommitSignRequestsAfterInstall(
                     request.node, request.tc):
                 /\ signRequest.vote.signer = signRequest.node
                 /\ signRequest.vote \in commitIntents'
                 /\ VoteRoundAdmissible(
                      signRequest.node, signRequest.vote)'
    <2> DEFINE Node == request.node
    <2> DEFINE Certificate == request.tc
    <2> DEFINE ResultingRank ==
          ResultingInstallLockRank(Node, Certificate)
    <2> DEFINE ResultingSubject ==
          ResultingInstallLockSubject(Node, Certificate)
    <2>1. /\ request \in pendingInstallTC
           /\ Node \in ValidatorIds
           /\ commitIntents \subseteq VoteRecordSet
      BY <1>1, Isa
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             PersistInstallTC, Node
    <2>2. /\ (ResultingRank = NoRank
                => ResultingSubject = NoSubject)
           /\ (ResultingRank # NoRank
                => \E qc \in prepareQCs:
                     /\ qc.context = context
                     /\ qc.view = ResultingRank
                     /\ qc.phase = "Prepare"
                     /\ qc.subject = ResultingSubject)
      BY <1>1, <2>1, PendingInstallTCResultingLockIsCertified
         DEF ResultingRank, ResultingSubject, Node, Certificate
    <2>3. /\ context' = context
           /\ commitIntents' = commitIntents
           /\ prepareQCs' = prepareQCs
           /\ lockRank'[Node] = ResultingRank
           /\ lockSubject'[Node] = ResultingSubject
      BY <1>1, <2>1, Isa
         DEF PersistInstallTC,
             ResultingInstallLockRank,
             ResultingInstallLockSubject,
             ResultingRank, ResultingSubject, Node, Certificate
    <2>4. ASSUME NEW signRequest \in
                     ActiveLockedCommitSignRequestsAfterInstall(
                       request.node, request.tc)
           PROVE /\ signRequest.vote.signer = signRequest.node
                 /\ signRequest.vote \in commitIntents'
                 /\ VoteRoundAdmissible(
                      signRequest.node, signRequest.vote)'
      <3>1. /\ signRequest.node = Node
             /\ signRequest.vote =
                  Vote(context, ResultingRank, "Commit",
                       ResultingSubject, Node)
             /\ signRequest.vote \in commitIntents
        BY <2>4, Isa
           DEF ActiveLockedCommitSignRequestsAfterInstall,
               ExactLockedCommitIntents, VoteSign,
               ResultingRank, ResultingSubject, Node, Certificate
      <3>2. /\ signRequest.vote.view \in Views
             /\ ResultingRank # NoRank
        BY <2>1, <3>1, Isa
           DEF VoteRecordSet, Vote, Views, Ranks, NoRank
      <3>3. PICK qc \in prepareQCs:
               /\ qc.context = context
               /\ qc.view = ResultingRank
               /\ qc.phase = "Prepare"
               /\ qc.subject = ResultingSubject
        BY <2>2, <3>2
      <3>4. LockedPrepareRound(
               signRequest.node, signRequest.vote.view,
               signRequest.vote.subject)'
        BY <2>3, <3>1, <3>3, Isa
           DEF LockedPrepareRound, Vote
      <3> QED BY <2>3, <3>1, <3>4, Isa
           DEF VoteRoundAdmissible, Vote
    <2> QED BY <2>4
  <1> QED BY <1>1

THEOREM RemovingOneSerializedBusyOwnerSeparatesRemainingNodes ==
  \A oldOwner:
    /\ SerializedBusyOwnershipInvariant
    /\ oldOwner \in SerializedBusyOwners
    => \A otherOwner \in SerializedBusyOwners \ {oldOwner}:
         otherOwner.node # oldOwner.node
BY DistinctUniqueRequestsHaveDistinctNodes, SMT
   DEF SerializedBusyOwnershipInvariant

THEOREM CorePersistInstallTCPreservesSerializedBusyKernel ==
  \A request:
    /\ StrongInductiveInvariant
    /\ AsyncSerializedBusyKernelInvariant
    /\ PersistInstallTC(request)
    => AsyncSerializedBusyKernelInvariant'
PROOF
  <1>1. ASSUME NEW request,
                StrongInductiveInvariant,
                AsyncSerializedBusyKernelInvariant,
                PersistInstallTC(request)
         PROVE AsyncSerializedBusyKernelInvariant'
    <2> DEFINE Node == request.node
    <2> DEFINE Certificate == request.tc
    <2> DEFINE NewOwners ==
          ActiveLockedCommitSignRequestsAfterInstall(Node, Certificate)
    <2>1. /\ request \in pendingInstallTC
           /\ request \in SerializedBusyOwners
           /\ SerializedBusyOwnershipInvariant
      BY <1>1, Isa
         DEF PersistInstallTC,
             AsyncSerializedBusyKernelInvariant,
             SerializedBusyOwners, AllPendingRequests
    <2>2. /\ RequestsUniqueByNode(NewOwners)
           /\ \A newOwner \in NewOwners:
                newOwner.node = Node
      BY ActiveLockedCommitSignRequestsAfterInstallIsUniqueAtNode
         DEF NewOwners
    <2>3. SerializedBusyOwners' =
             (SerializedBusyOwners \ {request}) \cup NewOwners
      BY <1>1, <2>1, InstallKindExcludesOtherWalSets, IsaT(120)
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             PersistInstallTC,
             SerializedBusyOwners, AllPendingRequests,
             ProposalWalSet, PrepareWalSet, ObservePrepareWalSet,
             LockCommitWalSet, TimeoutWalSet, InstallTcWalSet,
             DecisionWalSet, ProposalSignSet, VoteSignSet,
             TimeoutSignSet, NewOwners, Node, Certificate
    <2>4. SerializedBusyOwnershipInvariant'
      BY <2>1, <2>2, <2>3,
         ReplacingSerializedBusyOwnerBySameNodeSetPreservesOwnership
         DEF Node
    <2>5. \A otherOwner \in
                   SerializedBusyOwners \ {request}:
             otherOwner.node # Node
      BY <2>1, RemovingOneSerializedBusyOwnerSeparatesRemainingNodes
         DEF Node
    <2>6. \A signRequest \in NewOwners:
             /\ signRequest.vote.signer = signRequest.node
             /\ signRequest.vote \in commitIntents'
             /\ VoteRoundAdmissible(
                  signRequest.node, signRequest.vote)'
      BY <1>1, PersistInstallActiveSignRequestsAreReady
         DEF NewOwners, Node, Certificate
    <2>7. AsyncBusyReadinessInvariant'
      BY <1>1, <2>5, <2>6, IsaT(240)
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             AsyncSerializedBusyKernelInvariant,
             AsyncBusyReadinessInvariant,
             SerializedBusyOwners, AllPendingRequests,
             PersistInstallTC,
             VoteRoundAdmissible, LockedPrepareRound,
             NewOwners, Node, Certificate
    <2> QED BY <2>4, <2>7
         DEF AsyncSerializedBusyKernelInvariant
  <1> QED BY <1>1

THEOREM CoreSerializedBusyInstallActionPreservesKernel ==
  /\ StrongInductiveInvariant
  /\ AsyncSerializedBusyKernelInvariant
  /\ CoreSerializedBusyInstallAction
  => AsyncSerializedBusyKernelInvariant'
BY CorePersistInstallTCPreservesSerializedBusyKernel
   DEF CoreSerializedBusyInstallAction

(***************************************************************************
This direct action-preservation leaf for the serialized Busy kernel
deliberately assumes only the Core strong invariant, the kernel itself, and
the bracketed Core step; it must not be discharged from the later
progress-ownership induction.  The proof classifies all 47 atomic Core actions
into exact frame, monotone evidence growth, owner creation, same-node
conversion, owner removal, crash restriction, and InstallTC replacement, with
bracket stutter handled separately.  Fresh pinned strict sealing remains
pending for this direct split.
***************************************************************************)
THEOREM CoreNextPreservesAsyncSerializedBusyKernel ==
  /\ StrongInductiveInvariant
  /\ AsyncSerializedBusyKernelInvariant
  /\ [Next]_vars
  => AsyncSerializedBusyKernelInvariant'
BY CoreVarsStutterPreservesSerializedBusyKernelInvariant,
   CoreNextSerializedBusyActionClassification,
   CoreSerializedBusyExactFrameActionPreservesKernel,
   CoreSerializedBusyEvidenceGrowthActionPreservesKernel,
   CoreSerializedBusyOwnerCreationActionPreservesKernel,
   CoreSerializedBusyOwnerConversionActionPreservesKernel,
   CoreSerializedBusyOwnerRemovalActionPreservesKernel,
   CoreSerializedBusyCrashActionPreservesKernel,
   CoreSerializedBusyInstallActionPreservesKernel,
   Isa
   DEF vars

THEOREM AsyncNextPreservesSerializedBusyKernelInvariant ==
  /\ StrongInductiveInvariant
  /\ AsyncSerializedBusyKernelInvariant
  /\ AsyncNext
  => AsyncSerializedBusyKernelInvariant'
BY AsyncStepRefinementObligation,
   CoreNextPreservesAsyncSerializedBusyKernel

AsyncStrongTypeInvariant ==
  /\ StrongInductiveInvariant
  /\ AsyncSchedulerTypeInvariant
  /\ AsyncControlServiceStateTypeInvariant
  /\ AsyncCertifiedResponseClaimIngressOwnershipInvariant
  /\ ReceivedTimeoutVotePoolInvariant
  /\ AsyncRecoveryTypeInvariant
  /\ AsyncRestartAuthorityInvariant
  /\ AsyncRecoveryExecutionInvariant
  /\ AsyncHistoricalLockRestartAuthorityTypeInvariant
  /\ HistoricalLockRestartAuthoritySourceRetentionInvariant
  /\ AsyncGstRecoveryPhaseInvariant
  /\ AsyncSerializedBusyKernelInvariant

THEOREM AsyncStrongTypeProjectsAsyncType ==
  AsyncStrongTypeInvariant => AsyncTypeInvariant
BY DEF AsyncStrongTypeInvariant, AsyncTypeInvariant,
       StrongInductiveInvariant, Safety

THEOREM AsyncInitEstablishesStrongTypeInvariant ==
  \A initialContext:
    AsyncInitAt(initialContext) => AsyncStrongTypeInvariant
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncInitAt(initialContext)
         PROVE AsyncStrongTypeInvariant
    <2>1. StrongInductiveInvariant
      BY <1>1, InitAtEstablishesStrongInductiveInvariant
         DEF AsyncInitAt, AsyncBaseInitAt
    <2>2. TypeInvariant
      BY <2>1 DEF StrongInductiveInvariant, Safety
    <2>3. AsyncSchedulerTypeInvariant
      BY <1>1, <2>2, AsyncInitEstablishesSchedulerType
    <2>3b. AsyncControlServiceStateTypeInvariant
      BY <1>1, Isa
         DEF AsyncInitAt, AsyncBaseInitAt, AsyncTransportInit,
             AsyncControlServiceStateTypeInvariant,
             AsyncControlServiceSlots,
             AsyncNextControlServiceOrdinal,
             AsyncCertifiedResponseClaimRecords,
             AsyncNextCertifiedResponseClaimOrdinal,
             AsyncCandidateServiceTombstones,
             AsyncNextCandidateServiceOrdinal,
             AsyncCandidateServiceTombstoneSet
    <2>3a. AsyncCertifiedResponseClaimIngressOwnershipInvariant
      BY <1>1, EmptyCertifiedResponseClaimHasIngressOwnership
         DEF AsyncInitAt, AsyncBaseInitAt, AsyncTransportInit
    <2>4. ReceivedTimeoutVotePoolInvariant
      BY <1>1, AsyncInitEstablishesTimeoutPoolInvariant
    <2>5. /\ AsyncRecoveryTypeInvariant
           /\ AsyncRestartAuthorityInvariant
           /\ AsyncRecoveryExecutionInvariant
           /\ AsyncHistoricalLockRestartAuthorityTypeInvariant
           /\ HistoricalLockRestartAuthoritySourceRetentionInvariant
      BY <1>1, <2>2, SMT
         DEF AsyncInitAt, AsyncBaseInitAt, AsyncRecoveryInit,
             AsyncRecoveryTypeInvariant, AsyncRestartAuthorityInvariant,
             AsyncRecoveryExecutionInvariant,
             AsyncHistoricalLockRestartAuthorityTypeInvariant,
             HistoricalLockRestartAuthoritySourceRetentionInvariant,
             AsyncRecoveryPhases, TypeInvariant, ModelConfiguration,
             QuorumConfiguration, ValidatorIds, Generations
    <2>6. AsyncGstRecoveryPhaseInvariant
      BY <1>1, Isa
         DEF AsyncInitAt, AsyncBaseInitAt, InitAt,
             AsyncGstRecoveryPhaseInvariant
    <2>7. AsyncSerializedBusyKernelInvariant
      BY <1>1, AsyncInitEstablishesSerializedBusyKernelInvariant
    <2> QED BY <2>1, <2>3, <2>3a, <2>3b, <2>4, <2>5, <2>6,
                <2>7
         DEF AsyncStrongTypeInvariant
  <1> QED BY <1>1

THEOREM AsyncNextPreservesStrongInductiveInvariant ==
  StrongInductiveInvariant /\ AsyncNext
    => StrongInductiveInvariant'
BY AsyncStepRefinementObligation,
   CoreStrongInductiveActionPreservation

THEOREM AsyncAllVarsStutterPreservesTimeoutPoolInvariant ==
  ReceivedTimeoutVotePoolInvariant /\ UNCHANGED AsyncAllVars
    => ReceivedTimeoutVotePoolInvariant'
PROOF
  <1>1. ASSUME ReceivedTimeoutVotePoolInvariant,
              UNCHANGED AsyncAllVars
         PROVE ReceivedTimeoutVotePoolInvariant'
    <2>1. /\ receivedTimeoutVotes' = receivedTimeoutVotes
           /\ context' = context
           /\ height' = height
           /\ prepareQCs' = prepareQCs
      BY <1>1, Isa DEF AsyncAllVars, vars, AsyncSchedulerVars
    <2>2. prepareQCs \subseteq prepareQCs'
      BY <2>1
    <2> QED BY <1>1, <2>1, <2>2,
                TimeoutPoolFramePreservesInvariant
  <1> QED BY <1>1

THEOREM AsyncAllVarsStutterPreservesStrongTypeInvariant ==
  AsyncStrongTypeInvariant /\ UNCHANGED AsyncAllVars
    => AsyncStrongTypeInvariant'
PROOF
  <1>1. ASSUME AsyncStrongTypeInvariant,
              UNCHANGED AsyncAllVars
         PROVE AsyncStrongTypeInvariant'
    <2>1. /\ StrongInductiveInvariant
           /\ AsyncSchedulerTypeInvariant
           /\ AsyncControlServiceStateTypeInvariant
           /\ AsyncCertifiedResponseClaimIngressOwnershipInvariant
           /\ ReceivedTimeoutVotePoolInvariant
           /\ AsyncRecoveryTypeInvariant
           /\ AsyncRestartAuthorityInvariant
           /\ AsyncRecoveryExecutionInvariant
           /\ AsyncHistoricalLockRestartAuthorityTypeInvariant
           /\ HistoricalLockRestartAuthoritySourceRetentionInvariant
           /\ AsyncGstRecoveryPhaseInvariant
           /\ AsyncSerializedBusyKernelInvariant
      BY <1>1 DEF AsyncStrongTypeInvariant
    <2>2. UNCHANGED vars
      BY <1>1, Isa DEF AsyncAllVars
    <2>3. StrongInductiveInvariant'
      BY <2>1, <2>2, CoreStrongInductiveActionPreservation
    <2>4. AsyncSchedulerTypeInvariant'
      BY <1>1, <2>1, AsyncAllVarsStutterPreservesSchedulerType
    <2>4b. AsyncControlServiceStateTypeInvariant'
      BY <1>1, <2>1, Isa
         DEF AsyncAllVars, AsyncSchedulerVars,
             AsyncControlServiceStateTypeInvariant,
             AsyncControlServiceSlots,
             AsyncNextControlServiceOrdinal,
             AsyncCertifiedResponseClaimRecords,
             AsyncNextCertifiedResponseClaimOrdinal,
             AsyncCandidateServiceTombstones,
             AsyncNextCandidateServiceOrdinal
    <2>4a. AsyncCertifiedResponseClaimIngressOwnershipInvariant'
      BY <1>1, <2>1,
         CertifiedResponseClaimIngressOwnershipStutter
         DEF AsyncAllVars, AsyncSchedulerVars
    <2>5. ReceivedTimeoutVotePoolInvariant'
      BY <1>1, <2>1,
         AsyncAllVarsStutterPreservesTimeoutPoolInvariant
    <2>6. /\ AsyncRecoveryTypeInvariant'
           /\ AsyncRestartAuthorityInvariant'
           /\ AsyncRecoveryExecutionInvariant'
           /\ AsyncHistoricalLockRestartAuthorityTypeInvariant'
           /\ HistoricalLockRestartAuthoritySourceRetentionInvariant'
      BY <1>1, Isa
         DEF AsyncAllVars, AsyncRecoveryVars,
             AsyncRecoveryTypeInvariant, AsyncRestartAuthorityInvariant,
             AsyncRecoveryExecutionInvariant,
             AsyncHistoricalLockRestartAuthorityTypeInvariant,
             HistoricalLockRestartAuthoritySourceRetentionInvariant
    <2>7. AsyncGstRecoveryPhaseInvariant'
      BY <1>1, <2>1, Isa
         DEF AsyncAllVars, AsyncRecoveryVars, vars,
             AsyncGstRecoveryPhaseInvariant
    <2>8. AsyncSerializedBusyKernelInvariant'
      BY <2>1, <2>2,
         CoreVarsStutterPreservesSerializedBusyKernelInvariant
    <2> QED BY <2>3, <2>4, <2>4a, <2>4b, <2>5, <2>6, <2>7,
                <2>8
         DEF AsyncStrongTypeInvariant
  <1> QED BY <1>1

THEOREM PreGstResponsiveCrashPreservesSchedulerType ==
  \A node \in ValidatorIds:
    AsyncSchedulerTypeInvariant /\ PreGstResponsiveCrash(node)
      => AsyncSchedulerTypeInvariant'
BY AsyncSchedulerStateStutterPreservesType, Isa
   DEF PreGstResponsiveCrash, Crash, AsyncSchedulerVars, vars

THEOREM PreGstResponsiveRestartPreservesSchedulerType ==
  AsyncSchedulerTypeInvariant /\ PreGstResponsiveRestart
    => AsyncSchedulerTypeInvariant'
BY AsyncSchedulerStateStutterPreservesType, Isa
   DEF PreGstResponsiveRestart, Restart, AsyncSchedulerVars, vars

THEOREM RemoveRetainedControlSourcePreservesType ==
  \A retained, voters, node:
    AsyncRetainedControlType(retained, voters)
      => AsyncRetainedControlType(
           {item \in retained: item.source # node}, voters)
PROOF
  <1>1. ASSUME NEW retained, NEW voters, NEW node,
                AsyncRetainedControlType(retained, voters)
         PROVE AsyncRetainedControlType(
                 {item \in retained: item.source # node}, voters)
    <2> DEFINE Cleared == {item \in retained: item.source # node}
    <2>1. /\ Cleared \subseteq retained
           /\ IsFiniteSet(Cleared)
           /\ \A item \in Cleared:
                /\ AsyncItemTyped(item)
                /\ item.kind \in AsyncControlKinds
      BY <1>1, FS_Subset DEF AsyncRetainedControlType, Cleared
    <2>2. \A source \in ValidatorIds,
                  controlClass \in AsyncControlKinds:
             LET retainedClass ==
                   RetainedClassItems(Cleared, source, controlClass)
             IN \/ retainedClass = {}
                \/ /\ Cardinality(retainedClass) <=
                         Cardinality(voters)
                   /\ {item.envelope.recipient:
                         item \in retainedClass} =
                        ControlRecipients(source, controlClass, voters)
                   /\ \A left, right \in retainedClass:
                        ControlView(left) = ControlView(right)
      <3>1. ASSUME NEW source \in ValidatorIds,
                    NEW controlClass \in AsyncControlKinds
             PROVE LET retainedClass ==
                         RetainedClassItems(
                           Cleared, source, controlClass)
                   IN \/ retainedClass = {}
                      \/ /\ Cardinality(retainedClass) <=
                               Cardinality(voters)
                         /\ {item.envelope.recipient:
                               item \in retainedClass} =
                              ControlRecipients(
                                source, controlClass, voters)
                         /\ \A left, right \in retainedClass:
                              ControlView(left) = ControlView(right)
        <4>1. CASE source = node
          BY <4>1, Isa DEF Cleared, RetainedClassItems
        <4>2. CASE source # node
          <5>1. RetainedClassItems(Cleared, source, controlClass) =
                   RetainedClassItems(retained, source, controlClass)
            BY <4>2, Isa DEF Cleared, RetainedClassItems
          <5> QED BY <1>1, <3>1, <5>1
               DEF AsyncRetainedControlType
        <4> QED BY <4>1, <4>2
      <3> QED BY <3>1
    <2> QED BY <2>1, <2>2 DEF AsyncRetainedControlType, Cleared
  <1> QED BY <1>1

THEOREM RestartHighestPrepareControlIsRetainable ==
  \A node \in ValidatorIds:
    AsyncTypeInvariant =>
      RetainableControlBatch(
        RestartHighestPrepareControl(node), CurrentVoters)
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds, AsyncTypeInvariant
         PROVE RetainableControlBatch(
                 RestartHighestPrepareControl(node), CurrentVoters)
    <2>1. CASE RestartHighestPrepareQCs(node) = {}
      BY <2>1 DEF RestartHighestPrepareControl,
                     RetainableControlBatch
    <2>2. CASE RestartHighestPrepareQCs(node) # {}
      <3> DEFINE Certificate ==
             CHOOSE qc \in RestartHighestPrepareQCs(node): TRUE
      <3>1. Certificate \in RestartHighestPrepareQCs(node)
        BY <2>2, FS_EmptySet, Zenon DEF Certificate
      <3>2. Certificate \in QcRecordSet
        BY <1>1, <3>1 DEF AsyncTypeInvariant, TypeInvariant,
                              RestartHighestPrepareQCs
      <3>3. RetainableControlBatch(
               QcOutbox(node, Certificate), CurrentVoters)
        BY <1>1, <3>2, QcOutboxIsRetainable
      <3>4. RestartHighestPrepareControl(node) =
               QcOutbox(node, Certificate)
        BY <2>2 DEF RestartHighestPrepareControl, Certificate
      <3> QED BY <3>3, <3>4
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM RestartDecisionControlIsRetainable ==
  \A node \in ValidatorIds:
    /\ StrongInductiveInvariant
    /\ AsyncTypeInvariant
    => RetainableControlBatch(
         RestartDecisionControl(node), CurrentVoters)
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                StrongInductiveInvariant,
                AsyncTypeInvariant
         PROVE RetainableControlBatch(
                 RestartDecisionControl(node), CurrentVoters)
    <2>1. CASE RestartDecisionQCs(node) = {}
      BY <2>1 DEF RestartDecisionControl, RetainableControlBatch
    <2>2. CASE RestartDecisionQCs(node) # {}
      <3> DEFINE Certificate ==
             CHOOSE qc \in RestartDecisionQCs(node): TRUE
      <3>1. Certificate \in RestartDecisionQCs(node)
        BY <2>2, FS_EmptySet, Zenon DEF Certificate
      <3>2. Certificate \in QcRecordSet
        BY <1>1, <3>1, SMT
           DEF RestartDecisionQCs, StrongInductiveInvariant,
               Safety, DecisionAgreement, TypeInvariant
      <3>3. RetainableControlBatch(
               QcOutbox(node, Certificate), CurrentVoters)
        BY <1>1, <3>2, QcOutboxIsRetainable
      <3>4. RestartDecisionControl(node) =
               QcOutbox(node, Certificate)
        BY <2>2 DEF RestartDecisionControl, Certificate
      <3> QED BY <3>3, <3>4
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM RestartLastTcControlIsRetainable ==
  \A node \in ValidatorIds:
    AsyncTypeInvariant =>
      RetainableControlBatch(RestartLastTCControl(node), CurrentVoters)
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds, AsyncTypeInvariant
         PROVE RetainableControlBatch(
                 RestartLastTCControl(node), CurrentVoters)
    <2>1. CASE RestartLastInstalledTCs(node) = {}
      BY <2>1 DEF RestartLastTCControl, RetainableControlBatch
    <2>2. CASE RestartLastInstalledTCs(node) # {}
      <3> DEFINE Certificate ==
             CHOOSE tc \in RestartLastInstalledTCs(node): TRUE
      <3>1. Certificate \in RestartLastInstalledTCs(node)
        BY <2>2, FS_EmptySet, Zenon DEF Certificate
      <3>2. Certificate \in TcRecordSet
        BY <1>1, <3>1, SMT
           DEF RestartLastInstalledTCs, RestartInstalledTCs,
               AsyncTypeInvariant, TypeInvariant, TcWellTyped
      <3>3. RetainableControlBatch(
               TcOutbox(node, Certificate), CurrentVoters)
        BY <1>1, <3>2, TcOutboxIsRetainable
      <3>4. RestartLastTCControl(node) =
               TcOutbox(node, Certificate)
        BY <2>2 DEF RestartLastTCControl, Certificate
      <3> QED BY <3>3, <3>4
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM RestartRetainedControlPreservesType ==
  \A node \in ValidatorIds:
    /\ StrongInductiveInvariant
    /\ AsyncTypeInvariant
    => AsyncRetainedControlType(
         RestartRetainedControl(node), CurrentVoters)
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                StrongInductiveInvariant,
                AsyncTypeInvariant
         PROVE AsyncRetainedControlType(
                 RestartRetainedControl(node), CurrentVoters)
    <2> DEFINE Cleared ==
           {item \in asyncRetainedControl: item.source # node}
    <2> DEFINE WithPrepare ==
           RememberedControl(
             Cleared, RestartHighestPrepareControl(node))
    <2> DEFINE WithDecision ==
           RememberedControl(
             WithPrepare, RestartDecisionControl(node))
    <2>1. AsyncRetainedControlType(
             asyncRetainedControl, CurrentVoters)
      BY <1>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncTransportTypeInvariant,
             AsyncTransportContentTypeInvariant,
             AsyncTransportHistoryTypeInvariant
    <2>2. AsyncRetainedControlType(Cleared, CurrentVoters)
      BY <2>1, RemoveRetainedControlSourcePreservesType DEF Cleared
    <2>3. RetainableControlBatch(
             RestartHighestPrepareControl(node), CurrentVoters)
      BY <1>1, RestartHighestPrepareControlIsRetainable
    <2>4. AsyncRetainedControlType(WithPrepare, CurrentVoters)
      BY <2>2, <2>3, RememberedControlPreservesRetainedType
         DEF WithPrepare
    <2>5. RetainableControlBatch(
             RestartDecisionControl(node), CurrentVoters)
      BY <1>1, RestartDecisionControlIsRetainable
    <2>6. AsyncRetainedControlType(WithDecision, CurrentVoters)
      BY <2>4, <2>5, RememberedControlPreservesRetainedType
         DEF WithDecision
    <2>7. RetainableControlBatch(
             RestartLastTCControl(node), CurrentVoters)
      BY <1>1, RestartLastTcControlIsRetainable
    <2>8. AsyncRetainedControlType(
             RememberedControl(
               WithDecision, RestartLastTCControl(node)),
             CurrentVoters)
      BY <2>6, <2>7, RememberedControlPreservesRetainedType
    <2>9. RestartRetainedControl(node) =
             RememberedControl(
               WithDecision, RestartLastTCControl(node))
      BY DEF RestartRetainedControl, Cleared, WithPrepare, WithDecision
    <2> QED BY <2>8, <2>9
  <1> QED BY <1>1

THEOREM PreGstResponsiveReplayPreservesSchedulerType ==
  /\ StrongInductiveInvariant
  /\ AsyncTypeInvariant
  /\ PreGstResponsiveReplay
  => AsyncSchedulerTypeInvariant'
BY RestartReplayIsTypedOwnedAndUnique,
   RestartRetainedControlPreservesType,
   FilterActiveRequestsAndClaimPreservesInvariant, SMTT(45), Isa
   DEF PreGstResponsiveReplay, RecoveryCoreReplay,
       ResetNodeSchedulerForRestart,
       AsyncSchedulerTypeInvariant, AsyncRuntimeTypeInvariant,
       AsyncRuntimeScalarTypeInvariant, AsyncCausalTypeInvariant,
       AsyncCommandQueueOwnership, AsyncCausalQueueOwnership,
       AsyncQueueTyped, AsyncIoTypeInvariant,
       AsyncIoTopologyTypeInvariant, AsyncIoContentTypeInvariant,
       AsyncIoQueueContentTypeInvariant, AsyncIoWorkContentTypeInvariant,
       AsyncIoCapacityTypeInvariant, AsyncDeferredTypeInvariant,
       AsyncDeferredTopologyTypeInvariant,
       AsyncDeferredContentTypeInvariant, AsyncTransportTypeInvariant,
       AsyncTransportClockTypeInvariant,
       AsyncTransportContentTypeInvariant,
       AsyncTransportHistoryTypeInvariant,
       AsyncCertifiedResponseClaimInvariant,
       AsyncPacketContentTypeInvariant, AsyncHeldChunksTypeInvariant,
       AsyncIngressTypeInvariant, AsyncIngressTopologyTypeInvariant,
       AsyncIngressCapacityTypeInvariant,
       AsyncIngressContentTypeInvariant, AsyncConfiguration,
       AsyncLocalSources, AsyncCommandClasses, AsyncIngressSources,
       IngressLane, IngressLaneDepth, IngressDepth,
       IngressProtectedSlotCountFor, IngressProtectedSourcesFor,
       IngressTimeoutVoteProtectedSourcesFor,
       IngressTransportCompletionProtectedSourcesFor,
       IngressContinuationProtectedSourcesFor,
       IngressLaneHasNonTimeoutProgressIn,
       IngressLaneHasTimeoutVoteIn,
       IngressLaneHasTransportCompletionIn,
       IngressAdmissionClass, IngressProgressKinds,
       IngressTransportCompletionKinds, SequenceSet,
       ResumeProposal, ResumeVote, ResumeTimeout,
       StrongInductiveInvariant, Safety, AsyncTypeInvariant

THEOREM FreshTypedOwnedReplayCandidateProperties ==
  \A node, candidate:
    /\ AsyncCandidateTyped(candidate)
    /\ candidate.node = node
    => /\ AsyncQueueTyped(FreshCandidateSequence(candidate))
       /\ AsyncCausalQueueOwnership(
            node, FreshCandidateSequence(candidate))
       /\ SequenceHasUniqueValues(FreshCandidateSequence(candidate))
       /\ Len(FreshCandidateSequence(candidate)) <= 1
PROOF
  <1>1. ASSUME NEW node, NEW candidate,
                AsyncCandidateTyped(candidate),
                candidate.node = node
         PROVE /\ AsyncQueueTyped(FreshCandidateSequence(candidate))
               /\ AsyncCausalQueueOwnership(
                    node, FreshCandidateSequence(candidate))
               /\ SequenceHasUniqueValues(
                    FreshCandidateSequence(candidate))
               /\ Len(FreshCandidateSequence(candidate)) <= 1
    <2>1. CASE CandidateScheduled(candidate)
      <3>1. FreshCandidateSequence(candidate) = <<>>
        BY <2>1 DEF FreshCandidateSequence
      <3> QED BY <3>1, EmptyReplayProperties
    <2>2. CASE ~CandidateScheduled(candidate)
      <3>1. /\ AsyncQueueTyped(<<candidate>>)
             /\ AsyncCausalQueueOwnership(node, <<candidate>>)
             /\ SequenceHasUniqueValues(<<candidate>>)
        BY <1>1, TypedOwnedSingletonIsReplay
      <3>2. /\ FreshCandidateSequence(candidate) = <<candidate>>
             /\ Len(<<candidate>>) = 1
        BY <2>2 DEF FreshCandidateSequence
      <3> QED BY <3>1, <3>2
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM FreshReplayCandidateIsDisjointFromScheduled ==
  \A candidate:
    SequenceSet(FreshCandidateSequence(candidate)) \cap
      (QueuedCandidates \cup DeferredCandidates \cup CausalCandidates
        \cup TrackedWorkCandidates) = {}
BY Isa
   DEF FreshCandidateSequence, CandidateScheduled, SequenceSet

THEOREM DriveResponsiveReplayHeadPreservesSchedulerType ==
  /\ StrongInductiveInvariant
  /\ AsyncTypeInvariant
  /\ AsyncRecoveryTypeInvariant
  /\ DriveResponsiveReplayHead
  => AsyncSchedulerTypeInvariant'
PROOF
  <1>1. ASSUME StrongInductiveInvariant,
              AsyncTypeInvariant,
              AsyncRecoveryTypeInvariant,
              DriveResponsiveReplayHead
         PROVE AsyncSchedulerTypeInvariant'
    <2> DEFINE Node == asyncRecoveryNode
    <2> DEFINE Candidate == Head(asyncRecoveryReplayQueue)
    <2> DEFINE Fresh == FreshCandidateSequence(Candidate)
    <2>1. /\ Node \in ValidatorIds
           /\ AsyncCausalTypeInvariant
           /\ AsyncQueueTyped(asyncRecoveryReplayQueue)
           /\ Len(asyncRecoveryReplayQueue) > 0
      BY <1>1, SMT
         DEF Node, AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant, AsyncRecoveryTypeInvariant,
             DriveResponsiveReplayHead, ModelConfiguration
    <2>2. /\ AsyncCandidateTyped(Candidate)
           /\ Candidate.node = Node
      BY <1>1, <2>1, TypedQueueTailFacts, SMT
         DEF Candidate, Node, AsyncRecoveryTypeInvariant,
             SequenceSet
    <2>3. /\ AsyncQueueTyped(Fresh)
           /\ AsyncCausalQueueOwnership(Node, Fresh)
           /\ SequenceHasUniqueValues(Fresh)
           /\ Len(Fresh) <= 1
      BY <2>2, FreshTypedOwnedReplayCandidateProperties DEF Fresh
    <2>4. asyncCausalQueues' =
             [asyncCausalQueues EXCEPT ![Node] = @ \o Fresh]
      BY <1>1 DEF DriveResponsiveReplayHead, Node, Candidate, Fresh
    <2>5. AsyncCausalTypeInvariant'
      BY <2>1, <2>3, <2>4,
         AppendOwnedCausalSuccessorsPreservesCausalType
    <2>6. /\ AsyncRuntimeScalarTypeInvariant'
           /\ AsyncIoTypeInvariant'
           /\ AsyncDeferredTypeInvariant'
           /\ AsyncTransportTypeInvariant'
           /\ AsyncIngressTypeInvariant'
      BY <1>1, Isa
         DEF DriveResponsiveReplayHead, RecoveryCoreReplay,
             ResumeProposal, ResumeVote, ResumeTimeout,
             AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant, AsyncIoTypeInvariant,
             AsyncDeferredTypeInvariant, AsyncTransportTypeInvariant,
             AsyncIngressTypeInvariant
    <2> QED BY <2>5, <2>6
         DEF AsyncSchedulerTypeInvariant, AsyncRuntimeTypeInvariant
  <1> QED BY <1>1

THEOREM FinishResponsiveReplayPreservesSchedulerType ==
  /\ StrongInductiveInvariant
  /\ AsyncTypeInvariant
  /\ AsyncRecoveryTypeInvariant
  /\ FinishResponsiveReplay
  => AsyncSchedulerTypeInvariant'
PROOF
  <1>1. ASSUME StrongInductiveInvariant,
              AsyncTypeInvariant,
              AsyncRecoveryTypeInvariant,
              FinishResponsiveReplay
         PROVE AsyncSchedulerTypeInvariant'
    <2> DEFINE Node == asyncRecoveryNode
    <2> DEFINE Runner == RestartRunnerAssembly(Node)
    <2>1. /\ Node \in ValidatorIds
           /\ TypeInvariant
           /\ AsyncCausalTypeInvariant
      BY <1>1, SMT
         DEF Node, FinishResponsiveReplay,
             AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant, ModelConfiguration
    <2>2. /\ AsyncQueueTyped(Runner)
           /\ AsyncCausalQueueOwnership(Node, Runner)
           /\ SequenceHasUniqueValues(Runner)
           /\ Len(Runner) <= 1
      BY <2>1, RestartRunnerAssemblyProperties DEF Runner
    <2>3. CASE Len(Runner) = 0
      <3>1. UNCHANGED asyncCausalQueues
        BY <1>1, <2>3 DEF FinishResponsiveReplay, Node, Runner
      <3> QED BY <2>1, <3>1, Isa DEF AsyncCausalTypeInvariant
    <2>4. CASE Len(Runner) > 0
      <3> DEFINE Candidate == Runner[1]
      <3> DEFINE Fresh == FreshCandidateSequence(Candidate)
      <3>1. /\ Len(Runner) = 1
             /\ AsyncCandidateTyped(Candidate)
             /\ Candidate.node = Node
        BY <2>2, <2>4, SMT
           DEF Candidate, AsyncQueueTyped,
               AsyncCausalQueueOwnership, SequenceSet
      <3>2. /\ AsyncQueueTyped(Fresh)
             /\ AsyncCausalQueueOwnership(Node, Fresh)
             /\ SequenceHasUniqueValues(Fresh)
             /\ Len(Fresh) <= 1
        BY <3>1, FreshTypedOwnedReplayCandidateProperties DEF Fresh
      <3>3. asyncCausalQueues' =
               [asyncCausalQueues EXCEPT ![Node] = @ \o Fresh]
        BY <1>1, <3>1
           DEF FinishResponsiveReplay, Node, Runner, Candidate, Fresh
      <3> QED BY <2>1, <3>2, <3>3,
                   AppendOwnedCausalSuccessorsPreservesCausalType
    <2>5. AsyncCausalTypeInvariant'
      BY <2>2, <2>3, <2>4, SMT
    <2>6. /\ AsyncRuntimeScalarTypeInvariant'
           /\ AsyncIoTypeInvariant'
           /\ AsyncDeferredTypeInvariant'
           /\ AsyncTransportTypeInvariant'
           /\ AsyncIngressTypeInvariant'
      BY <1>1, Isa
         DEF FinishResponsiveReplay,
             AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant, AsyncIoTypeInvariant,
             AsyncDeferredTypeInvariant, AsyncTransportTypeInvariant,
             AsyncIngressTypeInvariant
    <2> QED BY <2>5, <2>6
         DEF AsyncSchedulerTypeInvariant, AsyncRuntimeTypeInvariant
  <1> QED BY <1>1

THEOREM AsyncNextPreservesSchedulerType ==
  /\ StrongInductiveInvariant
  /\ AsyncTypeInvariant
  /\ AsyncRecoveryTypeInvariant
  /\ AsyncNext
  => AsyncSchedulerTypeInvariant'
PROOF
  <1>1. ASSUME StrongInductiveInvariant,
              AsyncTypeInvariant,
              AsyncRecoveryTypeInvariant,
              AsyncNext
         PROVE AsyncSchedulerTypeInvariant'
    <2>1. CASE AsyncNonCrashStep
      <3>1. CASE AsyncRunnerStep
        BY <1>1, <2>1, <3>1,
           AsyncRunnerStepPreservesSchedulerType
      <3>2. CASE AsyncNonRunnerStep
        BY <1>1, <2>1, <3>2,
           AsyncNonRunnerStepPreservesSchedulerType
      <3>3. CASE DriveResponsiveReplayHead
        BY <1>1, <3>3,
           DriveResponsiveReplayHeadPreservesSchedulerType
      <3>4. CASE FinishResponsiveReplay
        BY <1>1, <3>4,
           FinishResponsiveReplayPreservesSchedulerType
      <3>5. CASE RearmResponsiveRecovery
        BY <1>1, <3>5, AsyncSchedulerStateStutterPreservesType, Isa
           DEF RearmResponsiveRecovery, AsyncTypeInvariant
      <3> QED BY <2>1, <3>1, <3>2, <3>3, <3>4, <3>5
           DEF AsyncNonCrashStep
    <2>2. CASE \E node \in ValidatorIds: PreGstCrash(node)
      BY <1>1, <2>2, PreGstCrashPreservesSchedulerType
         DEF AsyncTypeInvariant
    <2>3. CASE \E node \in ValidatorIds: PreGstResponsiveCrash(node)
      BY <1>1, <2>3, PreGstResponsiveCrashPreservesSchedulerType
         DEF AsyncTypeInvariant
    <2>4. CASE PreGstResponsiveRestart
      BY <1>1, <2>4, PreGstResponsiveRestartPreservesSchedulerType
         DEF AsyncTypeInvariant
    <2>5. CASE PreGstResponsiveReplay
      BY <1>1, <2>5, PreGstResponsiveReplayPreservesSchedulerType
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4, <2>5 DEF AsyncNext
  <1> QED BY <1>1

THEOREM FinishResponsiveReplayPreservesRecoveryInvariants ==
  /\ StrongInductiveInvariant
  /\ AsyncRecoveryTypeInvariant
  /\ AsyncRestartAuthorityInvariant
  /\ FinishResponsiveReplay
  => /\ AsyncRecoveryTypeInvariant'
     /\ AsyncRestartAuthorityInvariant'
PROOF
  <1>1. ASSUME StrongInductiveInvariant,
              AsyncRecoveryTypeInvariant,
              AsyncRestartAuthorityInvariant,
              FinishResponsiveReplay
         PROVE /\ AsyncRecoveryTypeInvariant'
               /\ AsyncRestartAuthorityInvariant'
    <2> DEFINE Node == asyncRecoveryNode
    <2>1. TypeInvariant
      BY <1>1, StrongInvariantProjectsType
    <2>2. ModelConfiguration
      BY <2>1 DEF TypeInvariant
    <2>3. Responsive \subseteq ValidatorIds
      BY <2>2, ModelResponsiveValidators
    <2>4. /\ asyncRecoveryPhase = "Replaying"
           /\ Node \in Responsive \cap up
           /\ Responsive \subseteq up
      BY <1>1, Isa
         DEF FinishResponsiveReplay, AsyncRecoveryTypeInvariant, Node
    <2>5. generation \in [ValidatorIds -> Generations]
      BY <2>1 DEF TypeInvariant
    <2>6. /\ Node \in ValidatorIds
           /\ generation[Node] \in Generations
      BY <2>3, <2>4, <2>5, FunctionValueHasCodomain
    <2>7. /\ asyncRecoveryPhase' = "Recovered"
           /\ asyncRecoveryNode' = Node
           /\ asyncRecoveryGeneration' = generation[Node]
           /\ asyncRecoveryReplayQueue' = <<>>
           /\ up' = up
      BY <1>1, Isa
         DEF FinishResponsiveReplay, AsyncRecoveryOuterFrame, vars, Node
    <2>8. /\ AsyncQueueTyped(asyncRecoveryReplayQueue')
           /\ Len(asyncRecoveryReplayQueue') = 0
           /\ SequenceSet(asyncRecoveryReplayQueue') = {}
      BY <2>7, EmptyReplayProperties, Isa DEF SequenceSet
    <2>9. /\ asyncRecoveryPhase' \in AsyncRecoveryPhases
           /\ asyncRecoveryNode' \in ValidatorIds
           /\ asyncRecoveryGeneration' \in Generations
           /\ AsyncQueueTyped(asyncRecoveryReplayQueue')
           /\ Len(asyncRecoveryReplayQueue') <= 2
      BY <2>6, <2>7, <2>8, Isa DEF AsyncRecoveryPhases
    <2>10. /\ asyncRecoveryPhase' # "Eligible"
            /\ asyncRecoveryPhase' # "RestartRequired"
            /\ asyncRecoveryPhase' # "ReplayRequired"
            /\ asyncRecoveryPhase' # "Replaying"
            /\ Responsive \subseteq up'
      BY <2>4, <2>7, Isa
    <2>11. AsyncRecoveryTypeInvariant'
      BY <2>7, <2>8, <2>9, <2>10, Isa
         DEF AsyncRecoveryTypeInvariant
    <2>12. AsyncRestartAuthorityInvariant'
      BY <2>7, Isa DEF AsyncRestartAuthorityInvariant
    <2> QED BY <2>11, <2>12
  <1> QED BY <1>1

THEOREM RearmResponsiveRecoveryPreservesRecoveryInvariants ==
  /\ StrongInductiveInvariant
  /\ AsyncRecoveryTypeInvariant
  /\ AsyncRestartAuthorityInvariant
  /\ RearmResponsiveRecovery
  => /\ AsyncRecoveryTypeInvariant'
     /\ AsyncRestartAuthorityInvariant'
PROOF
  <1>1. ASSUME StrongInductiveInvariant,
              AsyncRecoveryTypeInvariant,
              AsyncRestartAuthorityInvariant,
              RearmResponsiveRecovery
         PROVE /\ AsyncRecoveryTypeInvariant'
               /\ AsyncRestartAuthorityInvariant'
    <2>1. TypeInvariant
      BY <1>1, StrongInvariantProjectsType
    <2>2. ModelConfiguration
      BY <2>1 DEF TypeInvariant
    <2>3. /\ 0 \in ValidatorIds
           /\ 0 \in Generations
      BY <2>2, SMT
         DEF ModelConfiguration, QuorumConfiguration,
             ValidatorIds, Generations
    <2>4. /\ asyncRecoveryPhase' = "Eligible"
           /\ asyncRecoveryNode' = 0
           /\ asyncRecoveryGeneration' = 0
           /\ asyncRecoveryReplayQueue' = <<>>
           /\ Responsive \subseteq up
           /\ up' = up
      BY <1>1, Isa DEF RearmResponsiveRecovery, vars
    <2>5. /\ AsyncQueueTyped(asyncRecoveryReplayQueue')
           /\ Len(asyncRecoveryReplayQueue') = 0
           /\ SequenceSet(asyncRecoveryReplayQueue') = {}
      BY <2>4, EmptyReplayProperties, Isa DEF SequenceSet
    <2>6. /\ asyncRecoveryPhase' \in AsyncRecoveryPhases
           /\ asyncRecoveryNode' \in ValidatorIds
           /\ asyncRecoveryGeneration' \in Generations
           /\ AsyncQueueTyped(asyncRecoveryReplayQueue')
           /\ Len(asyncRecoveryReplayQueue') <= 2
      BY <2>3, <2>4, <2>5, Isa DEF AsyncRecoveryPhases
    <2>7. /\ asyncRecoveryPhase' # "RestartRequired"
           /\ asyncRecoveryPhase' # "ReplayRequired"
           /\ asyncRecoveryPhase' # "Replaying"
           /\ asyncRecoveryPhase' # "Recovered"
           /\ Responsive \subseteq up'
      BY <2>4, Isa
    <2>8. AsyncRecoveryTypeInvariant'
      BY <2>4, <2>5, <2>6, <2>7, Isa
         DEF AsyncRecoveryTypeInvariant
    <2>9. AsyncRestartAuthorityInvariant'
      BY <2>4, Isa DEF AsyncRestartAuthorityInvariant
    <2> QED BY <2>8, <2>9
  <1> QED BY <1>1

THEOREM ReplayingOrdinaryStepPreservesRecoveryCorridor ==
  /\ StrongInductiveInvariant
  /\ AsyncTypeInvariant
  /\ AsyncRecoveryTypeInvariant
  /\ AsyncRecoveryExecutionInvariant
  /\ asyncRecoveryPhase = "Replaying"
  /\ (AsyncRunnerStep \/ AsyncNonRunnerStep)
  /\ UNCHANGED <<up, AsyncRecoveryVars>>
  => /\ (~NodeHasApplication(asyncRecoveryNode))'
     /\ (RestartDecisions(asyncRecoveryNode) = {})'
     /\ \A request \in asyncActiveRequests':
          \/ request.source # asyncRecoveryNode'
          \/ (RestartLockedCertifiedRequest(
                asyncRecoveryNode, request))'
     /\ \A candidate \in
          ResponsiveReplayScheduledCandidates(asyncRecoveryNode)':
          /\ (CandidateConsumerCurrent(candidate))'
          /\ \/ candidate \in
                   (SequenceSet(
                      RestartSignatureReplay(asyncRecoveryNode)))'
             \/ (RestartLockedBodyPipelineCandidate(
                   asyncRecoveryNode, candidate))'
BY RestartSignatureReplayCommandsAreSignatures,
   RestartLockedBodyReplayCandidateShape,
   RestartReplayReplayingCandidateShape,
   SMTT(180), Isa
   DEF AsyncRunnerStep, RunNode, RunHistoricalRecoveryNode,
       RunNodeWork, LocalAdmissionStep, AdmitProducerCompletion,
       AdmitCausalHead, IngressDrainStep, DrainFairIngressSelected,
       SerializedRuntimeStep, RuntimeStep, DeferredDrainStep,
       FifoRuntimeStep, DeferredTagStep, DeferredTimeoutStep,
       DeferredRetransmitStep, DirectTimeoutStep,
       DirectRetransmitStep, IdleRuntimeStep,
       RemoveNextDeferredCommand, RemoveNextNodeCommand,
       DeferCommand, DiscardCommand, AdvanceNextDeferredClass,
       ExecuteCommand, ExecuteRegularCommand, ExecuteDecisionFetch,
       ExecuteSignProposal, ExecuteSignVote, ExecuteFormPrepareQC,
       ExecuteSignTimeout, ExecutePersistInstall,
       ExecutePersistDecision, ExecuteRequestCertifiedBody,
       ExecuteApply, ExecuteCoreDelivery, ExecuteChunkDelivery,
       ExecuteRejectAuthenticatedJunk,
       PublishCertifiedRequests, CertifiedRequestOutbox,
       CertifiedRecoveryFetchFrontier, LockedPrepareFetchFrontier,
       AppendCausalSuccessors, FreshCommandSuccessors,
       CommandSuccessors, FreshCandidateSequence,
       CausalCandidate, AsyncCandidateFrom,
       AsyncNonRunnerStep, AsyncNetworkStep, AdmitIngressPacket,
       AdmitHiddenPacket, CoalesceHiddenPacket,
       ServiceIoWorker, ServiceHistoricalRecoveryIoWorker,
       EnqueueIoLocalControl, EnqueueHistoricalRecoveryIoLocalControl,
       OpenHistoricalRecovery,
       DirectCommitCertificateDiscoveryStep,
       DirectHistoricalCommitCertificateDiscoveryStep,
       CommitCertificateDiscoveryStepWork,
       ResponsiveReplayQuarantined, ResponsiveReplayDraining,
       RestartLockedCertifiedRequest,
       RestartLockedBodyPipelineCandidate,
       RestartLockedPrepareQCs, LockedPrepareRecoverySource,
       ResponsiveReplayScheduledCandidates,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, CandidateScheduled,
       CandidateConsumerCurrent, NodeHasApplication, RestartDecisions,
       AsyncRecoveryTypeInvariant,
       AsyncRecoveryExecutionInvariant, AsyncRecoveryVars,
       SequenceSet, vars

THEOREM AsyncNextPreservesRecoveryInvariants ==
  /\ StrongInductiveInvariant
  /\ AsyncTypeInvariant
  /\ AsyncRecoveryTypeInvariant
  /\ AsyncRestartAuthorityInvariant
  /\ AsyncRecoveryExecutionInvariant
  /\ AsyncNext
  => /\ AsyncRecoveryTypeInvariant'
     /\ AsyncRestartAuthorityInvariant'
PROOF
  <1>1. ASSUME StrongInductiveInvariant,
              AsyncTypeInvariant,
              AsyncRecoveryTypeInvariant,
              AsyncRestartAuthorityInvariant,
              AsyncRecoveryExecutionInvariant,
              AsyncNext
         PROVE /\ AsyncRecoveryTypeInvariant'
               /\ AsyncRestartAuthorityInvariant'
    <2>1. CASE AsyncNonCrashStep
      <3>1. CASE /\ (AsyncRunnerStep \/ AsyncNonRunnerStep)
                   /\ UNCHANGED <<up, AsyncRecoveryControlVars>>
        BY <1>1, <3>1, SMTT(120), Isa
           DEF AsyncRecoveryTypeInvariant,
               AsyncRestartAuthorityInvariant, AsyncRecoveryPhases,
               AsyncRecoveryVars, AsyncRunnerStep, AsyncNonRunnerStep,
               ResponsiveReplayScheduledCandidates,
               ResponsiveReplayQuarantined, ResponsiveReplayDraining,
               RestartLockedCertifiedRequest,
               RestartLockedBodyPipelineCandidate,
               NodeHasApplication, RestartDecisions,
               StrongInductiveInvariant, Safety, TypeInvariant,
               ReducerProvenanceInvariant,
               PendingCertificateWritesAuthorized, vars
      <3>2. CASE /\ DriveResponsiveReplayHead
                   /\ UNCHANGED up
        BY <1>1, <3>2, TypedQueueTailFacts,
           RestartSignatureReplayProperties, SMTT(120), Isa
           DEF DriveResponsiveReplayHead, RecoveryCoreReplay,
               AsyncRecoveryTypeInvariant,
               AsyncRestartAuthorityInvariant, AsyncRecoveryPhases,
               AsyncRecoveryVars, AsyncRecoveryLifecycleVars,
               ResumeProposal, ResumeVote, ResumeTimeout,
               RestartSignatureReplay,
               ResponsiveReplayScheduledCandidates,
               CandidateConsumerCurrent,
               NodeHasApplication, RestartDecisions,
               SequenceSet, vars
      <3>3. CASE /\ FinishResponsiveReplay
                   /\ UNCHANGED up
        BY <1>1, <3>3,
           FinishResponsiveReplayPreservesRecoveryInvariants
      <3>4. CASE /\ RearmResponsiveRecovery
                   /\ UNCHANGED up
        BY <1>1, <3>4,
           RearmResponsiveRecoveryPreservesRecoveryInvariants
      <3> QED BY <2>1, <3>1, <3>2, <3>3, <3>4
           DEF AsyncNonCrashStep
    <2>2. CASE \E node \in ValidatorIds: PreGstCrash(node)
      <3>1. PICK node \in ValidatorIds: PreGstCrash(node)
        BY <2>2
      <3> QED BY <1>1, <3>1, SMTT(60), Isa
           DEF PreGstCrash, Crash, AsyncSchedulerVars,
               AsyncRecoveryTypeInvariant,
               AsyncRestartAuthorityInvariant, AsyncRecoveryPhases,
               AsyncRecoveryVars, ResponsiveReplayScheduledCandidates,
               NodeIdle, PendingNodes, SigningNodes,
               CandidateConsumerCurrent, RestartSignatureReplay,
               NodeHasApplication, RestartDecisions, vars
    <2>3. CASE \E node \in ValidatorIds:
                  PreGstResponsiveCrash(node)
      <3>1. PICK node \in ValidatorIds:
                 PreGstResponsiveCrash(node)
        BY <2>3
      <3> QED BY <1>1, <3>1, SMTT(60), Isa
           DEF PreGstResponsiveCrash, Crash, AsyncSchedulerVars,
               AsyncRecoveryTypeInvariant,
               AsyncRestartAuthorityInvariant, AsyncRecoveryPhases,
               AsyncRecoveryVars, ResponsiveReplayScheduledCandidates,
               NodeIdle, PendingNodes, SigningNodes,
               CandidateConsumerCurrent, RestartSignatureReplay,
               Generations, ValidatorIds, vars
    <2>4. CASE PreGstResponsiveRestart
      BY <1>1, <2>4, SMTT(60), Isa
         DEF PreGstResponsiveRestart, Restart, AsyncSchedulerVars,
             AsyncRecoveryTypeInvariant,
             AsyncRestartAuthorityInvariant, AsyncRecoveryPhases,
             AsyncRecoveryVars, ResponsiveReplayScheduledCandidates,
             NodeIdle, PendingNodes, SigningNodes,
             CandidateConsumerCurrent, RestartSignatureReplay,
             Generations, ValidatorIds, vars
    <2>5. CASE PreGstResponsiveReplay
      BY <1>1, <2>5, RestartSignatureReplayProperties,
         RestartReplayIsTypedOwnedAndUnique, TypedQueueTailFacts,
         RestartReplayReplayingCandidateShape,
         SMTT(120), Isa
         DEF PreGstResponsiveReplay, RecoveryCoreReplay,
             ResetNodeSchedulerForRestart,
             AsyncRecoveryTypeInvariant,
             AsyncRestartAuthorityInvariant, AsyncRecoveryPhases,
             AsyncRecoveryVars, ResumeProposal, ResumeVote, ResumeTimeout,
             RestartSignatureReplay, RestartReplay,
             RestartLockedBodyReplay,
             RestartLockedBodyPipelineCandidate,
             RestartLockedCertifiedRequest, RestartLockedPrepareQCs,
             RestartDecisions,
             ResponsiveReplayScheduledCandidates,
             QueuedCandidates, DeferredCandidates, CausalCandidates,
             TrackedWorkCandidates, CandidateConsumerCurrent,
             AsyncQueueTyped, SequenceSet, vars
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4, <2>5 DEF AsyncNext
  <1> QED BY <1>1

THEOREM ExecuteCommandLeavesOutstandingTags ==
  \A command:
    ExecuteCommand(command) => UNCHANGED asyncOutstandingTags
BY Isa
   DEF ExecuteCommand, ExecuteRegularCommand, ExecuteDecisionFetch,
       ExecuteSignProposal, ExecuteSignVote, ExecuteFormPrepareQC,
       ExecuteSignTimeout, ExecutePersistInstall,
       ExecutePersistDecision, ExecuteRequestCertifiedBody,
       ExecuteApply, ExecuteCoreDelivery, ExecuteChunkDelivery,
       ExecuteRejectAuthenticatedJunk, AsyncAuxVars, vars

THEOREM DeferredDrainStepLeavesOutstandingTags ==
  \A node:
    DeferredDrainStep(node) => UNCHANGED asyncOutstandingTags
BY ExecuteCommandLeavesOutstandingTags, Isa
   DEF DeferredDrainStep, DeferCommand, DiscardCommand,
       LeaveCausalQueues, vars

THEOREM FifoRuntimeStepLeavesOutstandingTags ==
  \A node:
    FifoRuntimeStep(node) => UNCHANGED asyncOutstandingTags
BY ExecuteCommandLeavesOutstandingTags, Isa
   DEF FifoRuntimeStep, DeferCommand, DiscardCommand,
       LeaveCausalQueues, vars

THEOREM LocalAdmissionStepLeavesOutstandingTags ==
  \A node:
    LocalAdmissionStep(node) => UNCHANGED asyncOutstandingTags
BY Isa
   DEF LocalAdmissionStep, AdmitProducerCompletion, AdmitCausalHead,
       LeaveCausalQueues, vars

THEOREM IngressDrainStepLeavesOutstandingTags ==
  \A node:
    IngressDrainStep(node) => UNCHANGED asyncOutstandingTags
BY Isa DEF IngressDrainStep, DrainFairIngressSelected,
           LeaveCausalQueues, vars

THEOREM HistoricalRunnerLeavesOutstandingTags ==
  \A node:
    RunHistoricalServer(node) => UNCHANGED asyncOutstandingTags
BY Isa
   DEF RunHistoricalServer, DrainHistoricalIngressSelected,
       HistoricalIdleStep, vars

THEOREM RunNodeWorkOutstandingTagsFrame ==
  \A node:
    /\ AsyncTypeInvariant
    /\ RunNodeWork(node)
    =>
      \A other \in ValidatorIds \ {node}:
        asyncOutstandingTags'[other] = asyncOutstandingTags[other]
PROOF
  <1>1. ASSUME NEW node,
                AsyncTypeInvariant,
                RunNodeWork(node)
         PROVE \A other \in ValidatorIds \ {node}:
                 asyncOutstandingTags'[other] =
                   asyncOutstandingTags[other]
    <2>1. DOMAIN asyncOutstandingTags = ValidatorIds
      BY <1>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncTransportTypeInvariant,
             AsyncTransportClockTypeInvariant
    <2>2. CASE LocalAdmissionStep(node)
      BY <2>2, LocalAdmissionStepLeavesOutstandingTags
    <2>3. CASE IngressDrainStep(node)
      BY <2>3, IngressDrainStepLeavesOutstandingTags
    <2>4. CASE SerializedRuntimeStep(node)
      <3>1. CASE DeferredDrainStep(node)
        BY <3>1, DeferredDrainStepLeavesOutstandingTags
      <3>2. CASE FifoRuntimeStep(node)
        BY <3>2, FifoRuntimeStepLeavesOutstandingTags
      <3>3. CASE DeferredTagStep(node)
        BY <2>1, <3>3, FunctionalUpdateAwayFromKey, Isa
           DEF DeferredTagStep, DeferredTimeoutStep,
               DeferredRetransmitStep, vars
      <3>4. CASE DirectTimeoutStep(node)
        BY <2>1, <3>4, FunctionalUpdateAwayFromKey, Isa
           DEF DirectTimeoutStep, vars
      <3>5. CASE DirectRetransmitStep(node)
        BY <2>1, <3>5, FunctionalUpdateAwayFromKey, Isa
           DEF DirectRetransmitStep, vars
      <3>6. CASE IdleRuntimeStep(node)
        BY <3>6, Isa DEF IdleRuntimeStep, vars
      <3> QED BY <2>4, <3>1, <3>2, <3>3, <3>4, <3>5, <3>6
           DEF SerializedRuntimeStep, RuntimeStep
    <2> QED BY <1>1, <2>2, <2>3, <2>4 DEF RunNodeWork
  <1> QED BY <1>1

THEOREM AsyncFaultStepLeavesOutstandingTags ==
  AsyncFaultStep => UNCHANGED asyncOutstandingTags
PROOF
  <1>1. ASSUME AsyncFaultStep
         PROVE UNCHANGED asyncOutstandingTags
    <2>1. CASE \E packet \in asyncTransport: PreGstLosePacket(packet)
      BY <2>1, Isa DEF PreGstLosePacket, vars
    <2>2. CASE \E node \in ValidatorIds: PreGstCrash(node)
      BY <2>2, Isa DEF PreGstCrash, Crash, AsyncSchedulerVars
    <2>3. CASE \E source \in AsyncIngressSources,
                  recipient \in ValidatorIds,
                  nonce \in 0..(AsyncIngressCapacity - 1):
                  InjectByzantineNoise(source, recipient, nonce)
      BY <2>3, Isa DEF InjectByzantineNoise, vars
    <2>3c. CASE \E kind \in IngressTransportCompletionKinds,
                   recipient \in ValidatorIds,
                   nonce \in 0..(AsyncIngressCapacity - 1):
                   InjectUntrustedTransportCompletion(
                     kind, recipient, nonce)
      BY <2>3c, Isa
         DEF InjectUntrustedTransportCompletion, vars
    <2>4. CASE \E kind \in {"NormalJunk", "ProgressJunk"},
                  source \in ValidatorIds, recipient \in ValidatorIds,
                  nonce \in 0..(AsyncIngressCapacity - 1):
                  InjectAuthenticatedJunk(kind, source, recipient, nonce)
      BY <2>4, Isa DEF InjectAuthenticatedJunk, vars
    <2>5. CASE \E source \in ValidatorIds, recipient \in ValidatorIds,
                  qc \in commitQCs,
                  nonce \in 0..(AsyncIngressCapacity - 1):
                  InjectByzantineCertifiedRequest(
                    source, recipient, qc, nonce)
      BY <2>5, Isa DEF InjectByzantineCertifiedRequest, vars
    <2>6. CASE \E signer \in ValidatorIds, roundView \in Views,
                  subject \in Subjects,
                  timeoutCertificate \in TimeoutCertificateOptionSet,
                  highestPrepare \in PrepareQcOptionSet:
                  AsyncByzantineProposal(
                    signer, roundView, subject,
                    timeoutCertificate, highestPrepare)
      BY <2>6, Isa DEF AsyncByzantineProposal
    <2>7. CASE \E signer \in ValidatorIds, roundView \in Views,
                  phase \in Phases, subject \in Subjects:
                  AsyncByzantineVote(signer, roundView, phase, subject)
      BY <2>7, Isa DEF AsyncByzantineVote
    <2>8. CASE \E signer \in ValidatorIds, roundView \in Views,
                  highestPrepare \in PrepareQcOptionSet:
                  AsyncByzantineTimeout(
                    signer, roundView, highestPrepare)
      BY <2>8, Isa DEF AsyncByzantineTimeout
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>3c, <2>4, <2>5,
                <2>6, <2>7, <2>8
         DEF AsyncFaultStep
  <1> QED BY <1>1

THEOREM AsyncNonRunnerStepLeavesOutstandingTags ==
  AsyncNonRunnerStep => UNCHANGED asyncOutstandingTags
PROOF
  <1>1. ASSUME AsyncNonRunnerStep
         PROVE UNCHANGED asyncOutstandingTags
    <2>1. CASE AsyncSetGST
      BY <2>1, Isa DEF AsyncSetGST, AsyncSchedulerVars
    <2>2. CASE AsyncTick
      BY <2>2, Isa DEF AsyncTick, AsyncNonClockVars
    <2>3. CASE \E node \in ValidatorIds:
                  OpenHistoricalRecovery(node)
      BY <2>3, Isa
         DEF OpenHistoricalRecovery,
             AsyncSchedulerExceptHistoricalRecoveryTargets
    <2>4. CASE \E node \in AsyncCurrentResponsiveVoters:
                  DirectCommitCertificateDiscoveryStep(node)
      BY <2>4, Isa
         DEF DirectCommitCertificateDiscoveryStep,
             CommitCertificateDiscoveryStepWork
    <2>5. CASE \E node \in asyncHistoricalRecoveryTargets:
                  DirectHistoricalCommitCertificateDiscoveryStep(node)
      BY <2>5, Isa
         DEF DirectHistoricalCommitCertificateDiscoveryStep,
             CommitCertificateDiscoveryStepWork
    <2>6. CASE \E node \in AsyncArchiveIoServiceNodes:
                  ServiceIoWorker(node)
      BY <2>6, Isa DEF ServiceIoWorker, ServiceIoWorkerWork
    <2>7. CASE \E node \in asyncHistoricalRecoveryTargets:
                  ServiceHistoricalRecoveryIoWorker(node)
      BY <2>7, Isa
         DEF ServiceHistoricalRecoveryIoWorker, ServiceIoWorkerWork
    <2>8. CASE \E node \in AsyncCurrentResponsiveVoters:
                  EnqueueIoLocalControl(node)
      BY <2>8, Isa
         DEF EnqueueIoLocalControl, EnqueueIoLocalControlWork
    <2>9. CASE \E node \in asyncHistoricalRecoveryTargets:
                  EnqueueHistoricalRecoveryIoLocalControl(node)
      BY <2>9, Isa
         DEF EnqueueHistoricalRecoveryIoLocalControl,
             EnqueueIoLocalControlWork
    <2>10. CASE AsyncNetworkStep
      BY <2>10, Isa
         DEF AsyncNetworkStep, AdmitIngressPacket,
             AdmitHiddenPacket, CoalesceHiddenPacket
    <2>11. CASE AsyncFaultStep
      BY <2>11, AsyncFaultStepLeavesOutstandingTags
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4, <2>5, <2>6,
                <2>7, <2>8, <2>9, <2>10, <2>11
         DEF AsyncNonRunnerStep
  <1> QED BY <1>1

AsyncRecoveryScheduledVars ==
  <<asyncCommandQueues, asyncOutstandingWork,
    asyncDeferredCompletionQueues, asyncDeferredProgressQueues,
    asyncDeferredNormalQueues, asyncCausalQueues>>

THEOREM RecoveryScheduledVarsStutterPreservesReplayFreshness ==
  \A node:
    UNCHANGED AsyncRecoveryScheduledVars
      => ResponsiveReplayScheduledCandidates(node)' =
           ResponsiveReplayScheduledCandidates(node)
BY Isa
   DEF AsyncRecoveryScheduledVars,
       ResponsiveReplayScheduledCandidates,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, SequenceSet

THEOREM RunHistoricalServerLeavesRecoveryScheduledVars ==
  \A node:
    RunHistoricalServer(node) => UNCHANGED AsyncRecoveryScheduledVars
BY IsaM("blast")
   DEF RunHistoricalServer, DrainHistoricalIngressSelected,
       HistoricalIdleStep, AsyncRecoveryScheduledVars,
       AsyncDeferredVars

THEOREM AsyncFaultStepLeavesRecoveryScheduledVars ==
  AsyncFaultStep => UNCHANGED AsyncRecoveryScheduledVars
BY IsaM("blast")
   DEF AsyncFaultStep, PreGstLosePacket, PreGstCrash,
       InjectByzantineNoise, InjectUntrustedTransportCompletion,
       InjectAuthenticatedJunk, InjectByzantineCertifiedRequest,
       AsyncByzantineProposal, AsyncByzantineVote,
       AsyncByzantineTimeout, AsyncSchedulerVars,
       AsyncRecoveryScheduledVars, AsyncDeferredVars,
       LeaveCausalQueues

THEOREM AsyncNonRunnerStepLeavesRecoveryScheduledVars ==
  AsyncNonRunnerStep => UNCHANGED AsyncRecoveryScheduledVars
BY AsyncFaultStepLeavesRecoveryScheduledVars, IsaM("blast")
   DEF AsyncNonRunnerStep, AsyncSetGST, AsyncTick,
       AsyncNonClockVars, OpenHistoricalRecovery,
       DirectCommitCertificateDiscoveryStep,
       DirectHistoricalCommitCertificateDiscoveryStep,
       CommitCertificateDiscoveryStepWork,
       PublishCommitCertificateRequests,
       ServiceIoWorker, ServiceHistoricalRecoveryIoWorker,
       ServiceIoWorkerWork,
       EnqueueIoLocalControl, EnqueueHistoricalRecoveryIoLocalControl,
       EnqueueIoLocalControlWork, AsyncNetworkStep,
       AdmitIngressPacket, AdmitHiddenPacket, CoalesceHiddenPacket,
       AsyncSchedulerVars, AsyncRecoveryScheduledVars,
       AsyncDeferredVars, LeaveCausalQueues

THEOREM ReplayingLocalAdmissionDoesNotCreateRecoveryCandidate ==
  \A node:
    /\ AsyncTypeInvariant
    /\ AsyncRecoveryTypeInvariant
    /\ asyncRecoveryPhase = "Replaying"
    /\ LocalAdmissionStep(node)
    => ResponsiveReplayScheduledCandidates(asyncRecoveryNode)'
         \subseteq
           ResponsiveReplayScheduledCandidates(asyncRecoveryNode)
BY HeadTailProperties, SequenceSetAfterAppend, SMTT(30), Isa
   DEF LocalAdmissionStep, AdmitProducerCompletion, AdmitCausalHead,
       UpdateLocalAdmissionMetadata, RecordBlockedCausalDebt,
       SelectedCompletionSource, SelectedCompletionCandidate,
       SelectedCompletionQueueNonempty, ProducerCompletionCanAdvance,
       LocalAdmissionCanAdvance, SelectedLocalSource,
       EnqueueCandidate, CausalHeadCanAdvance, CandidateInFlight,
       HeadCausalCandidate, ResponsiveReplayScheduledCandidates,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, AsyncTypeInvariant,
       AsyncSchedulerTypeInvariant, AsyncRuntimeTypeInvariant,
       AsyncRuntimeScalarTypeInvariant, AsyncCausalTypeInvariant,
       AsyncIoTypeInvariant, AsyncIoTopologyTypeInvariant,
       AsyncIoContentTypeInvariant, AsyncIoWorkContentTypeInvariant,
       AsyncRecoveryTypeInvariant, AsyncCommandQueueOwnership,
       AsyncCausalQueueOwnership, SequenceSet, vars

THEOREM ReplayingIngressDrainDoesNotCreateRecoveryCandidate ==
  \A node:
    /\ AsyncTypeInvariant
    /\ AsyncRecoveryTypeInvariant
    /\ asyncRecoveryPhase = "Replaying"
    /\ IngressDrainStep(node)
    => ResponsiveReplayScheduledCandidates(asyncRecoveryNode)'
         \subseteq
           ResponsiveReplayScheduledCandidates(asyncRecoveryNode)
BY SequenceSetAfterAppend, SMTT(30), Isa
   DEF IngressDrainStep, DrainFairIngressSelected,
       PopSelectedIngress, EnqueueCandidate,
       IngressItemCanDrain, DeliveryCandidate,
       CertifiedResponseCandidate,
       CommitCertificateResponseCandidate,
       AsyncIoCertifiedServeJob, AsyncIoJob,
       ResponsiveReplayScheduledCandidates,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, CandidateScheduled,
       AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
       AsyncCausalTypeInvariant,
       AsyncIoTypeInvariant, AsyncIoTopologyTypeInvariant,
       AsyncIoContentTypeInvariant, AsyncIoWorkContentTypeInvariant,
       AsyncDeferredTypeInvariant,
       AsyncDeferredTopologyTypeInvariant,
       AsyncDeferredContentTypeInvariant,
       AsyncIngressTypeInvariant, AsyncIngressContentTypeInvariant,
       AsyncRecoveryTypeInvariant, AsyncCommandQueueOwnership,
       AsyncCausalQueueOwnership, IngressLane, SequenceSet, vars

THEOREM ReplayingSerializedRuntimePreservesRecoveryCandidateFreshness ==
  \A node:
    /\ StrongInductiveInvariant
    /\ AsyncTypeInvariant
    /\ AsyncRecoveryTypeInvariant
    /\ AsyncRecoveryExecutionInvariant
    /\ asyncRecoveryPhase = "Replaying"
    /\ SerializedRuntimeStep(node)
    => SequenceSet(asyncRecoveryReplayQueue)' \cap
         ResponsiveReplayScheduledCandidates(asyncRecoveryNode)' = {}
BY HeadTailProperties, SequenceSetAfterAppend, SMTT(45), Isa
   DEF SerializedRuntimeStep, RuntimeStep,
       DeferredDrainStep, FifoRuntimeStep,
       DeferredTagStep, DeferredTimeoutStep,
       DeferredRetransmitStep, DirectTimeoutStep,
       DirectRetransmitStep, IdleRuntimeStep,
       RemoveNextDeferredCommand, RemoveNextNodeCommand,
       DeferCommand, DiscardCommand, AdvanceNextDeferredClass,
       ExecuteCommand, ExecuteRegularCommand, ExecuteDecisionFetch,
       ExecuteSignProposal, ExecuteSignVote, ExecuteFormPrepareQC,
       ExecuteSignTimeout, ExecutePersistInstall,
       ExecutePersistDecision, ExecuteRequestCertifiedBody,
       ExecuteApply, ExecuteCoreDelivery, ExecuteChunkDelivery,
       ExecuteRejectAuthenticatedJunk,
       AppendCausalSuccessors, FreshCommandSuccessors,
       CommandSuccessors, FreshCandidateSequence,
       CausalCandidate, AsyncCandidateFrom,
       CertifiedRecoveryFetchFrontier, DecisionFetchFrontier,
       LockedPrepareFetchFrontier,
       PersistDecisionRecoverySuccessor,
       PersistDecisionRecoveryKind, PersistDecisionValidationHeld,
       PersistDecisionBody, PersistDecisionRequest,
       PersistDecisionRequests,
       InstallCommandSuccessors, InstallLockedFetchSuccessors,
       InstallCommitSignSuccessors, InstallProposalSuccessor,
       ResponsiveReplayQuarantined,
       ResponsiveReplayScheduledCandidates,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, CandidateScheduled,
       AsyncRecoveryExecutionInvariant,
       AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
       AsyncCausalTypeInvariant,
       AsyncIoTypeInvariant, AsyncIoTopologyTypeInvariant,
       AsyncIoContentTypeInvariant, AsyncIoWorkContentTypeInvariant,
       AsyncDeferredTypeInvariant,
       AsyncDeferredTopologyTypeInvariant,
       AsyncDeferredContentTypeInvariant,
       AsyncRecoveryTypeInvariant, AsyncCommandQueueOwnership,
       AsyncCausalQueueOwnership,
       RestartLockedBodyPipelineCandidate,
       RestartLockedCertifiedRequest,
       RestartSignatureReplay, RestartTimeoutOrProposalReplay,
       RestartPrepareReplayIfActive, RestartLockedCommitReplayIfActive,
       RestartTimeoutReplay, RestartProposalReplay,
       RestartPrepareReplay, RestartLockedCommitReplay,
       RestartCandidate, AsyncCandidateAtConsumer,
       AsyncCandidateWithIdentity, CandidateConsumerCurrent,
       SequenceSet, vars

THEOREM ReplayingRunNodeWorkPreservesRecoveryCandidateFreshness ==
  \A node:
    /\ StrongInductiveInvariant
    /\ AsyncTypeInvariant
    /\ AsyncRecoveryTypeInvariant
    /\ AsyncRecoveryExecutionInvariant
    /\ asyncRecoveryPhase = "Replaying"
    /\ RunNodeWork(node)
    => SequenceSet(asyncRecoveryReplayQueue)' \cap
         ResponsiveReplayScheduledCandidates(asyncRecoveryNode)' = {}
BY ReplayingLocalAdmissionDoesNotCreateRecoveryCandidate,
   ReplayingIngressDrainDoesNotCreateRecoveryCandidate,
   ReplayingSerializedRuntimePreservesRecoveryCandidateFreshness,
   Isa
   DEF RunNodeWork, AsyncRecoveryExecutionInvariant,
       AsyncRecoveryVars, vars

THEOREM AsyncRunnerStepPreservesReplayCandidateFreshness ==
  /\ StrongInductiveInvariant
  /\ AsyncTypeInvariant
  /\ AsyncRecoveryTypeInvariant
  /\ AsyncRecoveryExecutionInvariant
  /\ asyncRecoveryPhase = "Replaying"
  /\ AsyncRunnerStep
  => SequenceSet(asyncRecoveryReplayQueue)' \cap
       ResponsiveReplayScheduledCandidates(asyncRecoveryNode)' = {}
PROOF
  <1>1. ASSUME StrongInductiveInvariant,
              AsyncTypeInvariant,
              AsyncRecoveryTypeInvariant,
              AsyncRecoveryExecutionInvariant,
              asyncRecoveryPhase = "Replaying",
              AsyncRunnerStep
         PROVE SequenceSet(asyncRecoveryReplayQueue)' \cap
                 ResponsiveReplayScheduledCandidates(
                   asyncRecoveryNode)' = {}
    <2>1. /\ UNCHANGED AsyncRecoveryControlVars
           /\ SequenceSet(asyncRecoveryReplayQueue) \cap
                ResponsiveReplayScheduledCandidates(
                  asyncRecoveryNode) = {}
      BY <1>1
         DEF AsyncRunnerStep, RunNode, RunHistoricalRecoveryNode,
             AsyncRecoveryExecutionInvariant
    <2>2. CASE \E node \in AsyncCurrentResponsiveVoters: RunNode(node)
      <3>1. PICK node \in AsyncCurrentResponsiveVoters: RunNode(node)
        BY <2>2
      <3>2. RunNodeWork(node)
        BY <3>1 DEF RunNode
      <3>3. SequenceSet(asyncRecoveryReplayQueue)' \cap
               ResponsiveReplayScheduledCandidates(
                 asyncRecoveryNode)' = {}
        BY <1>1, <3>2,
           ReplayingRunNodeWorkPreservesRecoveryCandidateFreshness
      <3> QED BY <3>3
    <2>3. CASE \E node \in asyncHistoricalRecoveryTargets:
                  RunHistoricalRecoveryNode(node)
      <3>1. PICK node \in asyncHistoricalRecoveryTargets:
               RunHistoricalRecoveryNode(node)
        BY <2>3
      <3>2. RunNodeWork(node)
        BY <3>1 DEF RunHistoricalRecoveryNode
      <3>3. SequenceSet(asyncRecoveryReplayQueue)' \cap
               ResponsiveReplayScheduledCandidates(
                 asyncRecoveryNode)' = {}
        BY <1>1, <3>2,
           ReplayingRunNodeWorkPreservesRecoveryCandidateFreshness
      <3> QED BY <3>3
    <2>4. CASE \E node \in AsyncResponsiveAppliedArchiveServers:
                  RunHistoricalServer(node)
      <3>1. PICK node \in AsyncResponsiveAppliedArchiveServers:
               RunHistoricalServer(node)
        BY <2>4
      <3>2. UNCHANGED AsyncRecoveryScheduledVars
        BY <3>1, RunHistoricalServerLeavesRecoveryScheduledVars
      <3>3. ResponsiveReplayScheduledCandidates(asyncRecoveryNode)' =
               ResponsiveReplayScheduledCandidates(asyncRecoveryNode)
        BY <3>2, RecoveryScheduledVarsStutterPreservesReplayFreshness
      <3> QED BY <2>1, <3>3, Isa DEF AsyncRecoveryVars
    <2> QED BY <1>1, <2>2, <2>3, <2>4 DEF AsyncRunnerStep
  <1> QED BY <1>1

THEOREM ReplayingRecoveryNodeSerializedRuntimePreservesEmptyTags ==
  \A node:
    /\ AsyncRecoveryExecutionInvariant
    /\ asyncRecoveryPhase = "Replaying"
    /\ node = asyncRecoveryNode
    /\ SerializedRuntimeStep(node)
    => asyncOutstandingTags'[node] = {}
PROOF
  <1>1. ASSUME NEW node,
                AsyncRecoveryExecutionInvariant,
                asyncRecoveryPhase = "Replaying",
                node = asyncRecoveryNode,
                SerializedRuntimeStep(node)
         PROVE asyncOutstandingTags'[node] = {}
    <2>1. asyncOutstandingTags[node] = {}
      BY <1>1 DEF AsyncRecoveryExecutionInvariant
    <2>2. CASE DeferredDrainStep(node)
      BY <2>1, <2>2, DeferredDrainStepLeavesOutstandingTags
    <2>3. CASE FifoRuntimeStep(node)
      BY <2>1, <2>3, FifoRuntimeStepLeavesOutstandingTags
    <2>4. CASE DeferredTagStep(node)
      BY <2>1, <2>4, Isa
         DEF DeferredTagStep, DeferredTimeoutStep,
             DeferredRetransmitStep, DeferredTimeoutExecutable
    <2>5. CASE DirectTimeoutStep(node)
      BY <1>1, <2>5, Isa
         DEF DirectTimeoutStep, TimeoutDue,
             ResponsiveReplayQuarantined
    <2>6. CASE DirectRetransmitStep(node)
      BY <1>1, <2>6, Isa
         DEF DirectRetransmitStep, RetransmitDue,
             ResponsiveReplayQuarantined
    <2>7. CASE IdleRuntimeStep(node)
      BY <2>1, <2>7, Isa DEF IdleRuntimeStep, vars
    <2> QED BY <1>1, <2>2, <2>3, <2>4, <2>5, <2>6, <2>7
         DEF SerializedRuntimeStep, RuntimeStep
  <1> QED BY <1>1

THEOREM ReplayingRecoveryNodeRunNodeWorkPreservesEmptyTags ==
  \A node:
    /\ AsyncRecoveryExecutionInvariant
    /\ asyncRecoveryPhase = "Replaying"
    /\ node = asyncRecoveryNode
    /\ RunNodeWork(node)
    => asyncOutstandingTags'[node] = {}
PROOF
  <1>1. ASSUME NEW node,
                AsyncRecoveryExecutionInvariant,
                asyncRecoveryPhase = "Replaying",
                node = asyncRecoveryNode,
                RunNodeWork(node)
         PROVE asyncOutstandingTags'[node] = {}
    <2>1. asyncOutstandingTags[node] = {}
      BY <1>1 DEF AsyncRecoveryExecutionInvariant
    <2>2. CASE LocalAdmissionStep(node)
      BY <2>1, <2>2, LocalAdmissionStepLeavesOutstandingTags
    <2>3. CASE IngressDrainStep(node)
      BY <2>1, <2>3, IngressDrainStepLeavesOutstandingTags
    <2>4. CASE SerializedRuntimeStep(node)
      BY <1>1, <2>4,
         ReplayingRecoveryNodeSerializedRuntimePreservesEmptyTags
    <2> QED BY <1>1, <2>2, <2>3, <2>4 DEF RunNodeWork
  <1> QED BY <1>1

THEOREM ReplayingRunNodeWorkPreservesRecoveryTags ==
  \A node:
    /\ AsyncTypeInvariant
    /\ AsyncRecoveryTypeInvariant
    /\ AsyncRecoveryExecutionInvariant
    /\ asyncRecoveryPhase = "Replaying"
    /\ RunNodeWork(node)
    => asyncOutstandingTags'[asyncRecoveryNode] = {}
PROOF
  <1>1. ASSUME NEW node,
                AsyncTypeInvariant,
                AsyncRecoveryTypeInvariant,
                AsyncRecoveryExecutionInvariant,
                asyncRecoveryPhase = "Replaying",
                RunNodeWork(node)
         PROVE asyncOutstandingTags'[asyncRecoveryNode] = {}
    <2>1. asyncRecoveryNode \in ValidatorIds
      BY <1>1 DEF AsyncRecoveryTypeInvariant
    <2>2. asyncOutstandingTags[asyncRecoveryNode] = {}
      BY <1>1 DEF AsyncRecoveryExecutionInvariant
    <2>3. CASE node = asyncRecoveryNode
      BY <1>1, <2>3,
         ReplayingRecoveryNodeRunNodeWorkPreservesEmptyTags
    <2>4. CASE node # asyncRecoveryNode
      <3>1. asyncRecoveryNode \in ValidatorIds \ {node}
        BY <2>1, <2>4
      <3>2. asyncOutstandingTags'[asyncRecoveryNode] =
               asyncOutstandingTags[asyncRecoveryNode]
        BY <1>1, <3>1, RunNodeWorkOutstandingTagsFrame
      <3> QED BY <2>2, <3>2
    <2> QED BY <2>3, <2>4
  <1> QED BY <1>1

THEOREM AsyncRunnerStepPreservesReplayingRecoveryTags ==
  /\ AsyncTypeInvariant
  /\ AsyncRecoveryTypeInvariant
  /\ AsyncRecoveryExecutionInvariant
  /\ asyncRecoveryPhase = "Replaying"
  /\ AsyncRunnerStep
  => asyncOutstandingTags'[asyncRecoveryNode] = {}
PROOF
  <1>1. ASSUME AsyncTypeInvariant,
              AsyncRecoveryTypeInvariant,
              AsyncRecoveryExecutionInvariant,
              asyncRecoveryPhase = "Replaying",
              AsyncRunnerStep
         PROVE asyncOutstandingTags'[asyncRecoveryNode] = {}
    <2>1. CASE \E node \in AsyncCurrentResponsiveVoters: RunNode(node)
      <3>1. PICK node \in AsyncCurrentResponsiveVoters: RunNode(node)
        BY <2>1
      <3>2. RunNodeWork(node)
        BY <3>1 DEF RunNode
      <3> QED BY <1>1, <3>2,
           ReplayingRunNodeWorkPreservesRecoveryTags
    <2>2. CASE \E node \in asyncHistoricalRecoveryTargets:
                  RunHistoricalRecoveryNode(node)
      <3>1. PICK node \in asyncHistoricalRecoveryTargets:
               RunHistoricalRecoveryNode(node)
        BY <2>2
      <3>2. RunNodeWork(node)
        BY <3>1 DEF RunHistoricalRecoveryNode
      <3> QED BY <1>1, <3>2,
           ReplayingRunNodeWorkPreservesRecoveryTags
    <2>3. CASE \E node \in AsyncResponsiveAppliedArchiveServers:
                  RunHistoricalServer(node)
      <3>1. PICK node \in AsyncResponsiveAppliedArchiveServers:
               RunHistoricalServer(node)
        BY <2>3
      <3>2. UNCHANGED asyncOutstandingTags
        BY <3>1, HistoricalRunnerLeavesOutstandingTags
      <3>3. asyncOutstandingTags[asyncRecoveryNode] = {}
        BY <1>1 DEF AsyncRecoveryExecutionInvariant
      <3> QED BY <3>2, <3>3
    <2> QED BY <1>1, <2>1, <2>2, <2>3 DEF AsyncRunnerStep
  <1> QED BY <1>1

THEOREM ResetNodeSchedulerForRestartSetsOutstandingTags ==
  \A node, replay:
    ResetNodeSchedulerForRestart(node, replay)
      => asyncOutstandingTags' =
           [asyncOutstandingTags EXCEPT ![node] = {}]
BY DEF ResetNodeSchedulerForRestart

THEOREM UniqueReplayTailPreservesUniqueValues ==
  \A queue:
    /\ queue \in Seq(Range(queue))
    /\ SequenceHasUniqueValues(queue)
    /\ Len(queue) > 0
    => SequenceHasUniqueValues(Tail(queue))
PROOF
  <1>1. ASSUME NEW queue,
                queue \in Seq(Range(queue)),
                SequenceHasUniqueValues(queue),
                Len(queue) > 0
         PROVE SequenceHasUniqueValues(Tail(queue))
    <2>1. IsInjective(queue)
      BY <1>1, UniqueSequenceLengthImpliesInjective
         DEF SequenceHasUniqueValues
    <2>2. /\ Tail(queue) \in Seq(Range(queue))
           /\ IsInjective(Tail(queue))
      BY <1>1, <2>1, EmptySeq, HeadTailProperties,
         TailInjectiveSeq, SMT
    <2>3. Tail(queue) \in Seq(Range(Tail(queue)))
      BY <2>2, SeqOfRange
    <2>4. Len(Tail(queue)) =
             Cardinality(SequenceSet(Tail(queue)))
      BY <2>2, <2>3, InjectiveSequenceLengthMatchesSetCardinality
    <2> QED BY <2>4 DEF SequenceHasUniqueValues
  <1> QED BY <1>1

THEOREM ReplayingRecoveryHeadIsFresh ==
  /\ AsyncRecoveryTypeInvariant
  /\ AsyncRecoveryExecutionInvariant
  /\ asyncRecoveryPhase = "Replaying"
  /\ Len(asyncRecoveryReplayQueue) > 0
  => LET candidate == Head(asyncRecoveryReplayQueue)
     IN /\ candidate \in SequenceSet(asyncRecoveryReplayQueue)
        /\ candidate.node = asyncRecoveryNode
        /\ ~CandidateScheduled(candidate)
        /\ FreshCandidateSequence(candidate) = <<candidate>>
PROOF
  <1>1. ASSUME AsyncRecoveryTypeInvariant,
              AsyncRecoveryExecutionInvariant,
              asyncRecoveryPhase = "Replaying",
              Len(asyncRecoveryReplayQueue) > 0
         PROVE LET candidate == Head(asyncRecoveryReplayQueue)
               IN /\ candidate \in
                        SequenceSet(asyncRecoveryReplayQueue)
                  /\ candidate.node = asyncRecoveryNode
                  /\ ~CandidateScheduled(candidate)
                  /\ FreshCandidateSequence(candidate) = <<candidate>>
    <2> DEFINE Candidate == Head(asyncRecoveryReplayQueue)
    <2>1. Candidate \in SequenceSet(asyncRecoveryReplayQueue)
      BY <1>1, PositiveSequenceIsNonempty,
         NonemptySequenceHeadIsFirst, SMT
         DEF AsyncRecoveryTypeInvariant, AsyncQueueTyped,
             Candidate, SequenceSet
    <2>2. Candidate.node = asyncRecoveryNode
      BY <1>1, <2>1 DEF AsyncRecoveryTypeInvariant, Candidate
    <2>3. ~CandidateScheduled(Candidate)
      BY <1>1, <2>1, <2>2, Isa
         DEF AsyncRecoveryExecutionInvariant,
             ResponsiveReplayScheduledCandidates, Candidate
    <2>4. FreshCandidateSequence(Candidate) = <<Candidate>>
      BY <2>3 DEF FreshCandidateSequence
    <2> QED BY <2>1, <2>2, <2>3, <2>4 DEF Candidate
  <1> QED BY <1>1

THEOREM DriveResponsiveReplayPreservesRecoveryExecutionInvariant ==
  /\ AsyncTypeInvariant
  /\ AsyncRecoveryTypeInvariant
  /\ AsyncRecoveryExecutionInvariant
  /\ DriveResponsiveReplayHead
  => AsyncRecoveryExecutionInvariant'
PROOF
  <1>1. ASSUME AsyncTypeInvariant,
              AsyncRecoveryTypeInvariant,
              AsyncRecoveryExecutionInvariant,
              DriveResponsiveReplayHead
         PROVE AsyncRecoveryExecutionInvariant'
    <2> DEFINE Node == asyncRecoveryNode
    <2> DEFINE Queue == asyncRecoveryReplayQueue
    <2> DEFINE Candidate == Head(Queue)
    <2>1. /\ asyncRecoveryPhase = "Replaying"
           /\ Len(Queue) > 0
           /\ Node \in ValidatorIds
           /\ AsyncQueueTyped(Queue)
           /\ SequenceHasUniqueValues(Queue)
           /\ SequenceSet(Queue) \cap
                ResponsiveReplayScheduledCandidates(Node) = {}
      BY <1>1
         DEF DriveResponsiveReplayHead,
             AsyncRecoveryTypeInvariant,
             AsyncRecoveryExecutionInvariant, Node, Queue
    <2>2. /\ Candidate \in SequenceSet(Queue)
           /\ Candidate.node = Node
           /\ ~CandidateScheduled(Candidate)
           /\ FreshCandidateSequence(Candidate) = <<Candidate>>
      BY <1>1, <2>1, ReplayingRecoveryHeadIsFresh
         DEF Node, Queue, Candidate
    <2>3. /\ asyncRecoveryPhase' = "Replaying"
           /\ asyncRecoveryNode' = Node
           /\ asyncRecoveryReplayQueue' = Tail(Queue)
           /\ UNCHANGED asyncOutstandingTags
      BY <1>1, Isa
         DEF DriveResponsiveReplayHead,
             AsyncRecoveryLifecycleVars, Node, Queue, Candidate
    <2>4. SequenceHasUniqueValues(asyncRecoveryReplayQueue')
      BY <2>1, <2>3, UniqueReplayTailPreservesUniqueValues
         DEF AsyncQueueTyped
    <2>5. SequenceSet(asyncRecoveryReplayQueue') =
             SequenceSet(Queue) \ {Candidate}
      <3>1. /\ Queue \in Seq(Range(Queue))
             /\ IsInjective(Queue)
             /\ Queue # <<>>
        BY <2>1, UniqueSequenceLengthImpliesInjective,
           PositiveSequenceIsNonempty
           DEF AsyncQueueTyped, SequenceHasUniqueValues
      <3>2. Range(Tail(Queue)) = Range(Queue) \ {Candidate}
        BY <3>1, TailInjectiveSeq DEF Candidate
      <3> QED BY <2>3, <3>1, <3>2, RangeEquality
           DEF SequenceSet
    <2>6. ResponsiveReplayScheduledCandidates(Node)' =
             ResponsiveReplayScheduledCandidates(Node) \cup {Candidate}
      BY <1>1, <2>1, <2>2,
         RangeConcatenation, RangeEquality, Isa
         DEF DriveResponsiveReplayHead,
             ResponsiveReplayScheduledCandidates,
             QueuedCandidates, DeferredCandidates, CausalCandidates,
             TrackedWorkCandidates, AsyncRecoveryLifecycleVars,
             FreshCandidateSequence, CandidateScheduled,
             AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant, AsyncCausalTypeInvariant,
             AsyncCausalQueueOwnership, SequenceSet,
             Node, Queue, Candidate
    <2>7. SequenceSet(asyncRecoveryReplayQueue)' \cap
             ResponsiveReplayScheduledCandidates(asyncRecoveryNode)' = {}
      BY <2>1, <2>2, <2>3, <2>5, <2>6, Isa
    <2>8. asyncOutstandingTags'[asyncRecoveryNode'] = {}
      BY <1>1, <2>1, <2>3
         DEF AsyncRecoveryExecutionInvariant
    <2> QED BY <2>3, <2>4, <2>7, <2>8
         DEF AsyncRecoveryExecutionInvariant
  <1> QED BY <1>1

THEOREM RestartSignatureTailIsFreshAgainstRestartReplay ==
  \A node \in ValidatorIds:
    /\ TypeInvariant
    /\ Len(RestartSignatureReplay(node)) > 0
    => SequenceSet(Tail(RestartSignatureReplay(node))) \cap
         SequenceSet(RestartReplay(node)) = {}
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                TypeInvariant,
                Len(RestartSignatureReplay(node)) > 0
         PROVE SequenceSet(Tail(RestartSignatureReplay(node))) \cap
                 SequenceSet(RestartReplay(node)) = {}
    <2> DEFINE Signatures == RestartSignatureReplay(node)
    <2> DEFINE Locked == RestartLockedBodyReplay(node)
    <2>1. /\ AsyncQueueTyped(Signatures)
           /\ SequenceHasUniqueValues(Signatures)
      BY <1>1, RestartSignatureReplayProperties DEF Signatures
    <2>2. /\ Signatures \in Seq(Range(Signatures))
           /\ IsInjective(Signatures)
           /\ Signatures # <<>>
      BY <1>1, <2>1, UniqueSequenceLengthImpliesInjective,
         PositiveSequenceIsNonempty
         DEF AsyncQueueTyped, SequenceHasUniqueValues, Signatures
    <2>3. /\ IsInjective(Tail(Signatures))
           /\ Range(Tail(Signatures)) =
                Range(Signatures) \ {Head(Signatures)}
      BY <2>2, TailInjectiveSeq
    <2>4. SequenceSet(Tail(Signatures)) =
             SequenceSet(Signatures) \ {Head(Signatures)}
      BY <2>2, <2>3, HeadTailProperties, SeqOfRange,
         RangeEquality, Isa DEF SequenceSet
    <2>5. SequenceSet(Tail(Signatures)) \cap
             SequenceSet(Locked) = {}
      BY <2>4, RestartLockedBodyAndSignatureReplayAreDisjoint, SMT
         DEF Locked, Signatures
    <2>6. Head(Signatures)
             \notin SequenceSet(Tail(Signatures))
      BY <2>4, Isa
    <2>7. RestartReplay(node) =
             Locked \o <<Head(Signatures)>>
      BY <1>1, <2>2, Isa
         DEF RestartReplay, RestartSignatureReplay, Signatures, Locked
    <2>8. SequenceSet(RestartReplay(node)) =
             SequenceSet(Locked) \cup {Head(Signatures)}
      BY <2>1, <2>2, <2>7, RangeConcatenation, RangeEquality,
         SingletonSequenceFacts, Isa
         DEF AsyncQueueTyped, SequenceSet
    <2> QED BY <2>5, <2>6, <2>8, Isa
  <1> QED BY <1>1

THEOREM PreGstResponsiveReplayEstablishesRecoveryExecutionInvariant ==
  /\ AsyncTypeInvariant
  /\ AsyncRecoveryTypeInvariant
  /\ PreGstResponsiveReplay
  => AsyncRecoveryExecutionInvariant'
PROOF
  <1>1. ASSUME AsyncTypeInvariant,
              AsyncRecoveryTypeInvariant,
              PreGstResponsiveReplay
         PROVE AsyncRecoveryExecutionInvariant'
    <2>1. asyncRecoveryNode \in ValidatorIds
      BY <1>1 DEF AsyncRecoveryTypeInvariant
    <2>2. DOMAIN asyncOutstandingTags = ValidatorIds
      BY <1>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncTransportTypeInvariant,
             AsyncTransportClockTypeInvariant
    <2>3. ResetNodeSchedulerForRestart(
             asyncRecoveryNode, RestartReplay(asyncRecoveryNode))
      BY <1>1 DEF PreGstResponsiveReplay
    <2>4. asyncOutstandingTags' =
             [asyncOutstandingTags EXCEPT ![asyncRecoveryNode] = {}]
      BY <2>3, ResetNodeSchedulerForRestartSetsOutstandingTags
    <2>5. asyncOutstandingTags'[asyncRecoveryNode] = {}
      BY <2>1, <2>2, <2>4, FunctionalReplaceUpdateAtKey
    <2>6. asyncRecoveryNode' = asyncRecoveryNode
      BY <1>1, Isa DEF PreGstResponsiveReplay
    <2>6a. SequenceHasUniqueValues(asyncRecoveryReplayQueue')
      <3> DEFINE Node == asyncRecoveryNode
      <3> DEFINE Signatures == RestartSignatureReplay(Node)
      <3>1. /\ AsyncQueueTyped(Signatures)
             /\ SequenceHasUniqueValues(Signatures)
        BY <1>1, RestartSignatureReplayProperties
           DEF AsyncTypeInvariant, StrongInductiveInvariant, Safety,
               AsyncRecoveryTypeInvariant, Node, Signatures
      <3>2. CASE Len(Signatures) = 0
        BY <1>1, <3>2, Isa
           DEF PreGstResponsiveReplay, Signatures,
               SequenceHasUniqueValues, SequenceSet
      <3>3. CASE Len(Signatures) > 0
        <4>1. SequenceHasUniqueValues(Tail(Signatures))
          BY <3>1, <3>3, UniqueReplayTailPreservesUniqueValues
             DEF AsyncQueueTyped
        <4> QED BY <1>1, <3>3, <4>1
             DEF PreGstResponsiveReplay, Signatures
      <3>4. Len(Signatures) = 0 \/ Len(Signatures) > 0
        BY <3>1, SMT DEF AsyncQueueTyped
      <3> QED BY <3>2, <3>3, <3>4
    <2>7. SequenceSet(asyncRecoveryReplayQueue)' \cap
             ResponsiveReplayScheduledCandidates(asyncRecoveryNode)' = {}
      <3> DEFINE Node == asyncRecoveryNode
      <3> DEFINE Signatures == RestartSignatureReplay(Node)
      <3>1. /\ TypeInvariant
             /\ AsyncSchedulerTypeInvariant
             /\ Node \in ValidatorIds
             /\ AsyncQueueTyped(Signatures)
             /\ AsyncCausalQueueOwnership(Node, Signatures)
             /\ SequenceHasUniqueValues(Signatures)
        BY <1>1, RestartSignatureReplayProperties
           DEF AsyncTypeInvariant, StrongInductiveInvariant, Safety,
               AsyncRecoveryTypeInvariant, Node, Signatures
      <3>2. CASE Len(Signatures) = 0
        BY <1>1, <3>2, Isa
           DEF PreGstResponsiveReplay, Signatures,
               ResponsiveReplayScheduledCandidates, SequenceSet
      <3>3. CASE Len(Signatures) > 0
        <4>1. SequenceSet(Tail(Signatures)) \cap
                 SequenceSet(RestartReplay(Node)) = {}
          BY <3>1, <3>3,
             RestartSignatureTailIsFreshAgainstRestartReplay
             DEF Node, Signatures
        <4>2. asyncRecoveryReplayQueue' = Tail(Signatures)
          BY <1>1, <3>3
             DEF PreGstResponsiveReplay, Node, Signatures
        <4>3. ResponsiveReplayScheduledCandidates(Node)' =
                 SequenceSet(RestartReplay(Node))
          BY <1>1, <3>1, Isa
             DEF PreGstResponsiveReplay, ResetNodeSchedulerForRestart,
                 ResponsiveReplayScheduledCandidates,
                 QueuedCandidates, DeferredCandidates, CausalCandidates,
                 TrackedWorkCandidates,
                 AsyncSchedulerTypeInvariant, AsyncRuntimeTypeInvariant,
                 AsyncRuntimeScalarTypeInvariant,
                 AsyncCommandQueueOwnership,
                 AsyncIoTypeInvariant, AsyncIoContentTypeInvariant,
                 AsyncIoWorkContentTypeInvariant,
                 AsyncDeferredTypeInvariant,
                 AsyncDeferredContentTypeInvariant,
                 AsyncCausalQueueOwnership, SequenceSet, Node
        <4> QED BY <2>6, <4>1, <4>2, <4>3
      <3>4. Len(Signatures) = 0 \/ Len(Signatures) > 0
        BY <3>1, SMT DEF AsyncQueueTyped
      <3> QED BY <3>2, <3>3, <3>4
    <2> QED BY <2>5, <2>6, <2>6a, <2>7
         DEF AsyncRecoveryExecutionInvariant
  <1> QED BY <1>1

THEOREM AsyncNextPreservesRecoveryExecutionInvariant ==
  /\ StrongInductiveInvariant
  /\ AsyncTypeInvariant
  /\ AsyncRecoveryTypeInvariant
  /\ AsyncRestartAuthorityInvariant
  /\ AsyncRecoveryExecutionInvariant
  /\ AsyncNext
  => AsyncRecoveryExecutionInvariant'
PROOF
  <1>1. ASSUME StrongInductiveInvariant,
              AsyncTypeInvariant,
              AsyncRecoveryTypeInvariant,
              AsyncRestartAuthorityInvariant,
              AsyncRecoveryExecutionInvariant,
              AsyncNext
         PROVE AsyncRecoveryExecutionInvariant'
    <2>1. CASE asyncRecoveryPhase' # "Replaying"
      BY <2>1 DEF AsyncRecoveryExecutionInvariant
    <2>2. CASE asyncRecoveryPhase' = "Replaying"
      <3>1. CASE AsyncNonCrashStep
        <4>1. CASE /\ (AsyncRunnerStep \/ AsyncNonRunnerStep)
                     /\ UNCHANGED <<up, AsyncRecoveryControlVars>>
          <5>1. /\ asyncRecoveryPhase = "Replaying"
                 /\ asyncRecoveryNode' = asyncRecoveryNode
            BY <2>2, <4>1, Isa DEF AsyncRecoveryVars
          <5>2. CASE AsyncRunnerStep
            <6>1. asyncOutstandingTags'[asyncRecoveryNode] = {}
              BY <1>1, <5>1, <5>2,
                 AsyncRunnerStepPreservesReplayingRecoveryTags
            <6>2. SequenceHasUniqueValues(
                     asyncRecoveryReplayQueue')
              BY <1>1, <4>1, <5>1
                 DEF AsyncRecoveryExecutionInvariant,
                     AsyncRecoveryVars
            <6>3. SequenceSet(asyncRecoveryReplayQueue)' \cap
                     ResponsiveReplayScheduledCandidates(
                       asyncRecoveryNode)' = {}
              BY <1>1, <5>1, <5>2,
                 AsyncRunnerStepPreservesReplayCandidateFreshness
            <6> QED BY <2>2, <5>1, <6>1, <6>2, <6>3
                 DEF AsyncRecoveryExecutionInvariant
          <5>3. CASE AsyncNonRunnerStep
            <6>1. UNCHANGED asyncOutstandingTags
              BY <5>3, AsyncNonRunnerStepLeavesOutstandingTags
            <6>2. asyncOutstandingTags[asyncRecoveryNode] = {}
              BY <1>1, <5>1 DEF AsyncRecoveryExecutionInvariant
            <6>3. SequenceHasUniqueValues(
                     asyncRecoveryReplayQueue')
              BY <1>1, <4>1, <5>1
                 DEF AsyncRecoveryExecutionInvariant,
                     AsyncRecoveryVars
            <6>4. UNCHANGED AsyncRecoveryScheduledVars
              BY <5>3, AsyncNonRunnerStepLeavesRecoveryScheduledVars
            <6>5. ResponsiveReplayScheduledCandidates(
                     asyncRecoveryNode)' =
                       ResponsiveReplayScheduledCandidates(
                         asyncRecoveryNode)
              BY <6>4,
                 RecoveryScheduledVarsStutterPreservesReplayFreshness
            <6>6. SequenceSet(asyncRecoveryReplayQueue)' \cap
                     ResponsiveReplayScheduledCandidates(
                       asyncRecoveryNode)' = {}
              BY <1>1, <4>1, <5>1, <6>5, Isa
                 DEF AsyncRecoveryExecutionInvariant,
                     AsyncRecoveryVars
            <6> QED BY <2>2, <5>1, <6>1, <6>2, <6>3, <6>6
                 DEF AsyncRecoveryExecutionInvariant
          <5> QED BY <4>1, <5>2, <5>3
        <4>2. CASE DriveResponsiveReplayHead
          BY <1>1, <4>2,
             DriveResponsiveReplayPreservesRecoveryExecutionInvariant
        <4>3. CASE FinishResponsiveReplay
          BY <2>2, <4>3, Isa DEF FinishResponsiveReplay
        <4>4. CASE RearmResponsiveRecovery
          BY <2>2, <4>4, Isa DEF RearmResponsiveRecovery
        <4> QED BY <3>1, <4>1, <4>2, <4>3, <4>4
             DEF AsyncNonCrashStep
      <3>2. CASE \E node \in ValidatorIds: PreGstCrash(node)
        <4>1. PICK node \in ValidatorIds: PreGstCrash(node)
          BY <3>2
        <4> QED BY <1>1, <2>2, <4>1, Isa
             DEF PreGstCrash, AsyncSchedulerVars, AsyncRecoveryVars,
                 AsyncRecoveryExecutionInvariant
      <3>3. CASE \E node \in ValidatorIds:
                    PreGstResponsiveCrash(node)
        BY <2>2, <3>3, Isa DEF PreGstResponsiveCrash
      <3>4. CASE PreGstResponsiveRestart
        BY <2>2, <3>4, Isa DEF PreGstResponsiveRestart
      <3>5. CASE PreGstResponsiveReplay
        BY <1>1, <3>5,
           PreGstResponsiveReplayEstablishesRecoveryExecutionInvariant
      <3> QED BY <1>1, <2>2, <3>1, <3>2, <3>3, <3>4, <3>5
           DEF AsyncNext
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM ResponsiveCrashRegistrationProjectsExistingDurableLock ==
  \A node \in ValidatorIds:
    /\ StrongInductiveInvariant
    /\ PreGstResponsiveCrash(node)
    => \A authority \in
           ResponsiveCrashHistoricalLockRestartAuthorities(node):
         HistoricalLockRestartAuthoritySourceAfter(authority)
BY SMTT(120), Isa
   DEF PreGstResponsiveCrash, Crash,
       ResponsiveCrashHistoricalLockRestartAuthorities,
       HistoricalLockRestartAuthoritySourceAfter,
       HistoricalLockRestartAuthoritySourceKernel,
       AsyncHistoricalLockRestartAuthority,
       StrongInductiveInvariant, Safety, TypeInvariant, vars

THEOREM AsyncNextPreservesHistoricalLockRestartAuthorityInvariants ==
  /\ StrongInductiveInvariant
  /\ AsyncHistoricalLockRestartAuthorityTypeInvariant
  /\ HistoricalLockRestartAuthoritySourceRetentionInvariant
  /\ AsyncNext
  => /\ AsyncHistoricalLockRestartAuthorityTypeInvariant'
     /\ HistoricalLockRestartAuthoritySourceRetentionInvariant'
PROOF
  <1>1. ASSUME StrongInductiveInvariant,
              AsyncHistoricalLockRestartAuthorityTypeInvariant,
              HistoricalLockRestartAuthoritySourceRetentionInvariant,
              AsyncNext
         PROVE /\ AsyncHistoricalLockRestartAuthorityTypeInvariant'
               /\ HistoricalLockRestartAuthoritySourceRetentionInvariant'
    <2>1. AsyncHistoricalLockRestartAuthorityTransition
      BY <1>1 DEF AsyncNext
    <2>2. CASE \E node \in ValidatorIds:
                  ResponsiveCrashRecoveryRegistration(node)
      BY <1>1, <2>1, <2>2,
         ResponsiveCrashRegistrationProjectsExistingDurableLock,
         SMTT(180), Isa
         DEF AsyncNext, AsyncNonCrashStep,
             ResponsiveCrashRecoveryRegistration,
             AsyncHistoricalLockRestartAuthorityTransition,
             AsyncHistoricalLockRestartAuthorityTypeInvariant,
             HistoricalLockRestartAuthoritySourceRetentionInvariant,
             HistoricalLockRestartAuthoritySource,
             HistoricalLockRestartAuthoritySourceAfter,
             AsyncHistoricalLockRestartAuthoritySet,
             PreGstResponsiveCrash, Crash, ValidatorIds, vars
    <2>3. CASE ~\E node \in ValidatorIds:
                   ResponsiveCrashRecoveryRegistration(node)
      BY <1>1, <2>1, <2>3, SMTT(120), Isa
         DEF AsyncHistoricalLockRestartAuthorityTransition,
             AsyncHistoricalLockRestartAuthorityTypeInvariant,
             HistoricalLockRestartAuthoritySourceRetentionInvariant,
             HistoricalLockRestartAuthoritySource,
             HistoricalLockRestartAuthoritySourceAfter
    <2> QED BY <2>2, <2>3
  <1> QED BY <1>1

THEOREM AsyncNextPreservesGstRecoveryPhaseInvariant ==
  /\ AsyncGstRecoveryPhaseInvariant
  /\ AsyncNext
  => AsyncGstRecoveryPhaseInvariant'
BY SMTT(60), Isa
   DEF AsyncGstRecoveryPhaseInvariant,
       AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       AsyncSetGST, SetGST, AsyncTick,
       OpenHistoricalRecovery,
       DirectCommitCertificateDiscoveryStep,
       DirectHistoricalCommitCertificateDiscoveryStep,
       CommitCertificateDiscoveryStepWork,
       ServiceIoWorker, ServiceHistoricalRecoveryIoWorker,
       EnqueueIoLocalControl, EnqueueHistoricalRecoveryIoLocalControl,
       AsyncNetworkStep, AsyncFaultStep,
       DriveResponsiveReplayHead, FinishResponsiveReplay,
       RearmResponsiveRecovery,
       PreGstCrash, PreGstResponsiveCrash,
       PreGstResponsiveRestart, PreGstResponsiveReplay,
       Crash, Restart, RecoveryCoreReplay,
       ResumeProposal, ResumeVote, ResumeTimeout,
       AsyncRecoveryVars, vars

THEOREM ResetNodeSchedulerPreservesClaimIngressOwnership ==
  \A node \in ValidatorIds:
  \A replay:
    /\ AsyncTypeInvariant
    /\ AsyncCertifiedResponseClaimIngressOwnershipInvariant
    /\ ResetNodeSchedulerForRestart(node, replay)
    => AsyncCertifiedResponseClaimIngressOwnershipInvariant'
BY SMTT(120), IsaT(180)
   DEF ResetNodeSchedulerForRestart,
       AsyncCertifiedResponseClaimIngressOwnershipInvariant,
       CertifiedResponseClaimIngressOwner,
       CertifiedResponseClaimForRequests,
       ActiveCertifiedRequestHashesIn,
       CertifiedResponseClaimProjectionAuthenticated,
       MatchingCertifiedRequests, FrozenCertifiedRequestRegistration,
       FrozenCertifiedResponseBinding,
       AsyncCertifiedResponseCanonicalWireIdentity,
       AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncIngressTypeInvariant, AsyncIngressTopologyTypeInvariant,
       AsyncIngressContentTypeInvariant,
       IngressLane, IngressLaneDepth, SequenceSet

THEOREM PreGstResponsiveReplayPreservesClaimIngressOwnership ==
  /\ AsyncTypeInvariant
  /\ AsyncCertifiedResponseClaimIngressOwnershipInvariant
  /\ PreGstResponsiveReplay
    => AsyncCertifiedResponseClaimIngressOwnershipInvariant'
BY ResetNodeSchedulerPreservesClaimIngressOwnership, Isa
   DEF PreGstResponsiveReplay, RecoveryCoreReplay

THEOREM AsyncNextPreservesCertifiedResponseClaimIngressOwnershipInvariant ==
  /\ AsyncTypeInvariant
  /\ AsyncCertifiedResponseClaimIngressOwnershipInvariant
  /\ AsyncNext
  => AsyncCertifiedResponseClaimIngressOwnershipInvariant'
PROOF
  <1>1. ASSUME AsyncTypeInvariant,
              AsyncCertifiedResponseClaimIngressOwnershipInvariant,
              AsyncNext
         PROVE AsyncCertifiedResponseClaimIngressOwnershipInvariant'
    <2>1. CASE AsyncNonCrashStep
      <3>1. CASE AsyncRunnerStep
        BY <1>1, <3>1,
           AsyncRunnerStepPreservesClaimIngressOwnership
      <3>2. CASE AsyncNonRunnerStep
        BY <1>1, <3>2,
           AsyncNonRunnerStepPreservesClaimIngressOwnership
      <3>3. CASE ~(AsyncRunnerStep \/ AsyncNonRunnerStep)
        BY <1>1, <2>1, <3>3,
           CertifiedResponseClaimIngressOwnershipStutter, Isa
           DEF AsyncNonCrashStep, DriveResponsiveReplayHead,
               FinishResponsiveReplay, RearmResponsiveRecovery,
               AsyncSchedulerVars
      <3> QED BY <2>1, <3>1, <3>2, <3>3
           DEF AsyncNonCrashStep
    <2>2. CASE \E node \in ValidatorIds: PreGstCrash(node)
      BY <1>1, <2>2,
         CertifiedResponseClaimIngressOwnershipStutter, Isa
         DEF PreGstCrash, AsyncSchedulerVars
    <2>3. CASE \E node \in ValidatorIds:
                    PreGstResponsiveCrash(node)
      BY <1>1, <2>3,
         CertifiedResponseClaimIngressOwnershipStutter, Isa
         DEF PreGstResponsiveCrash, AsyncSchedulerVars
    <2>4. CASE PreGstResponsiveRestart
      BY <1>1, <2>4,
         CertifiedResponseClaimIngressOwnershipStutter, Isa
         DEF PreGstResponsiveRestart, AsyncSchedulerVars
    <2>5. CASE PreGstResponsiveReplay
      BY <2>5,
         PreGstResponsiveReplayPreservesClaimIngressOwnership
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4, <2>5
         DEF AsyncNext
  <1> QED BY <1>1

THEOREM AsyncNextPreservesControlServiceStateTypeInvariant ==
  /\ AsyncTypeInvariant
  /\ AsyncControlServiceStateTypeInvariant
  /\ AsyncNext
  => AsyncControlServiceStateTypeInvariant'
BY FS_Subset, FS_Image, IsaT(600)
   DEF AsyncNext,
       AsyncControlServiceSlotTransition,
       AsyncControlServiceStateAfterReset,
       AsyncCandidateServiceTombstonesAfterReset,
       AsyncControlServiceStateAfterAdmission,
       AsyncControlServiceStateAfterService,
       AsyncCertifiedResponseClaimStateAfterRetirement,
       AsyncCertifiedResponseClaimStateAfterAdmission,
       AsyncCandidateServiceStateAfterReclamation,
       AsyncCandidateServiceStateAfterSuccessfulService,
       AsyncCandidateServiceStateAfterTerminalRetirement,
       AsyncControlServiceResetNodesThisStep,
       AsyncControlServiceAdmissionsThisStep,
       AsyncControlServicesThisStep,
       AsyncCertifiedResponseClaimAdmissionsThisStep,
       AsyncCandidateServicesThisStep,
       AsyncCandidateSuccessfullyServicedThisStep,
       AsyncCandidateTerminalRetirementsThisStep,
       AsyncCandidateTerminalDiscardsThisStep,
       AsyncCandidateTerminallyDiscardedThisStep,
       AsyncCandidateTerminalRetirementEligibleAfterStep,
       AsyncControlServiceStateTypeInvariant,
       AsyncControlServiceRecordSet,
       AsyncControlServiceRecord,
       AsyncControlServiceSlots,
       AsyncNextControlServiceOrdinal,
       AsyncCertifiedResponseClaimRecords,
       AsyncNextCertifiedResponseClaimOrdinal,
       AsyncCertifiedResponseClaimRecord,
       CertifiedResponseClaimRecordsFor,
       AsyncCandidateServiceTombstones,
       AsyncNextCandidateServiceOrdinal,
       AsyncCandidateServiceTombstoneSet,
       AsyncCandidateServiceTombstone,
       AsyncCandidateServiceIdentity,
       AsyncCandidateServicePayload,
       AsyncCandidateServiceRecordsForIdentity,
       AsyncCandidateServiceTombstoneRetainedAfterStep,
       AsyncCandidateServiceEligibleAfterStep,
       AsyncControlServiceSlotSet,
       AsyncControlServiceSlot,
       AsyncControlServiceProtocolOwner,
       AsyncControlServiceAdmissionStartsOrReplaces,
       AsyncControlServiceCurrentHeightItem,
       AsyncLeaderWireServiceIdentity

THEOREM AsyncNextPreservesStrongTypeInvariant ==
  AsyncStrongTypeInvariant /\ AsyncNext
    => AsyncStrongTypeInvariant'
PROOF
  <1>1. ASSUME AsyncStrongTypeInvariant,
              AsyncNext
         PROVE AsyncStrongTypeInvariant'
    <2>1. StrongInductiveInvariant
      BY <1>1 DEF AsyncStrongTypeInvariant
    <2>2. AsyncTypeInvariant
      BY <1>1, AsyncStrongTypeProjectsAsyncType
    <2>2a. AsyncRecoveryTypeInvariant
      BY <1>1 DEF AsyncStrongTypeInvariant
    <2>2b. AsyncRestartAuthorityInvariant
      BY <1>1 DEF AsyncStrongTypeInvariant
    <2>2c. AsyncRecoveryExecutionInvariant
      BY <1>1 DEF AsyncStrongTypeInvariant
    <2>2d. /\ AsyncHistoricalLockRestartAuthorityTypeInvariant
            /\ HistoricalLockRestartAuthoritySourceRetentionInvariant
      BY <1>1 DEF AsyncStrongTypeInvariant
    <2>2e. AsyncGstRecoveryPhaseInvariant
      BY <1>1 DEF AsyncStrongTypeInvariant
    <2>2f. AsyncSerializedBusyKernelInvariant
      BY <1>1 DEF AsyncStrongTypeInvariant
    <2>2g. AsyncCertifiedResponseClaimIngressOwnershipInvariant
      BY <1>1 DEF AsyncStrongTypeInvariant
    <2>2h. AsyncControlServiceStateTypeInvariant
      BY <1>1 DEF AsyncStrongTypeInvariant
    <2>3. StrongInductiveInvariant'
      BY <1>1, <2>1, AsyncNextPreservesStrongInductiveInvariant
    <2>4. AsyncSchedulerTypeInvariant'
      BY <1>1, <2>1, <2>2, <2>2a,
         AsyncNextPreservesSchedulerType
    <2>4b. AsyncControlServiceStateTypeInvariant'
      BY <1>1, <2>2, <2>2h,
         AsyncNextPreservesControlServiceStateTypeInvariant
    <2>5. ReceivedTimeoutVotePoolInvariant'
      BY <1>1, <2>2, AsyncNextPreservesTimeoutPoolInvariant
    <2>6. /\ AsyncRecoveryTypeInvariant'
           /\ AsyncRestartAuthorityInvariant'
      BY <1>1, <2>1, <2>2, <2>2a, <2>2b, <2>2c,
         AsyncNextPreservesRecoveryInvariants
    <2>7. AsyncRecoveryExecutionInvariant'
      BY <1>1, <2>1, <2>2, <2>2a, <2>2b, <2>2c,
         AsyncNextPreservesRecoveryExecutionInvariant
    <2>8. /\ AsyncHistoricalLockRestartAuthorityTypeInvariant'
           /\ HistoricalLockRestartAuthoritySourceRetentionInvariant'
      BY <1>1, <2>1, <2>2d,
         AsyncNextPreservesHistoricalLockRestartAuthorityInvariants
    <2>9. AsyncSerializedBusyKernelInvariant'
      BY <1>1, <2>1, <2>2f,
         AsyncNextPreservesSerializedBusyKernelInvariant
    <2>10. AsyncGstRecoveryPhaseInvariant'
      BY <1>1, <2>2e,
         AsyncNextPreservesGstRecoveryPhaseInvariant
    <2>11. AsyncCertifiedResponseClaimIngressOwnershipInvariant'
      BY <1>1, <2>2, <2>2g,
         AsyncNextPreservesCertifiedResponseClaimIngressOwnershipInvariant
    <2> QED BY <2>3, <2>4, <2>4b, <2>5, <2>6, <2>7, <2>8,
                <2>9, <2>10, <2>11
         DEF AsyncStrongTypeInvariant
  <1> QED BY <1>1

THEOREM AsyncBracketNextPreservesStrongTypeInvariant ==
  AsyncStrongTypeInvariant /\ [AsyncNext]_AsyncAllVars
    => AsyncStrongTypeInvariant'
PROOF
  <1>1. ASSUME AsyncStrongTypeInvariant,
              [AsyncNext]_AsyncAllVars
         PROVE AsyncStrongTypeInvariant'
    <2>1. CASE AsyncNext
      BY <1>1, <2>1, AsyncNextPreservesStrongTypeInvariant
    <2>2. CASE UNCHANGED AsyncAllVars
      BY <1>1, <2>2,
         AsyncAllVarsStutterPreservesStrongTypeInvariant
    <2> QED BY <1>1, <2>1, <2>2
  <1> QED BY <1>1

(***************************************************************************
Retained locked-body round rebinding.  View-independent retained authority may
survive a view change, but it is usable only by `RebindRetainedBody`; it is not
durable, validated, or applicable target-round evidence.  Proposal delivery
therefore emits a completion-class rebind candidate that materializes an exact
target-view Available record.  The ordinary StoreBody -> ValidateBody chain
then writes exact-view durable and validation evidence.
***************************************************************************)

RetainedBodyRebindReady(command) ==
  /\ command.kind = "RebindRetainedBody"
  /\ command.class = "Completion"
  /\ CandidateConsumerCurrent(command)
  /\ lockRank[command.node] # NoRank
  /\ lockSubject[command.node] = command.subject
  /\ RetainedLockedBodyHeldBy(
       retainedLockedBodies, command.node, context, command.subject)
  /\ BodyRecord(command.node, context, command.view, command.subject)
       \in BodyRecordSet
  /\ BodyRecord(command.node, context, command.view, command.subject)
       \notin availableBodies
  /\ \E proposal \in SeenProposalValues:
       /\ CommandMatches(command, command.node, proposal.view,
                         proposal.subject)
       /\ ProposalAt(command.node, proposal) \in seenProposals

RetainedBodyRebindAction(command, proposal) ==
  /\ command.kind = "RebindRetainedBody"
  /\ CommandMatches(command, command.node, proposal.view,
                    proposal.subject)
  /\ RebindRetainedBody(command.node, proposal)
  /\ UNCHANGED AsyncAuxVars

THEOREM RetainedBodyRebindCandidateIsTypedAndOwned ==
  \A command:
    (AsyncTypeInvariant /\ AsyncCandidateTyped(command))
      => /\ AsyncCandidateTyped(
               RetainedBodyRebindCandidate(command))
         /\ RetainedBodyRebindCandidate(command)
              \in AsyncCandidateSet
         /\ RetainedBodyRebindCandidate(command).node = command.node
         /\ RetainedBodyRebindCandidate(command).class = "Completion"
         /\ RetainedBodyRebindCandidate(command).kind =
              "RebindRetainedBody"
PROOF
  <1>1. ASSUME NEW command,
                AsyncTypeInvariant,
                AsyncCandidateTyped(command)
         PROVE /\ AsyncCandidateTyped(
                      RetainedBodyRebindCandidate(command))
                /\ RetainedBodyRebindCandidate(command)
                     \in AsyncCandidateSet
                /\ RetainedBodyRebindCandidate(command).node =
                     command.node
                /\ RetainedBodyRebindCandidate(command).class =
                     "Completion"
                /\ RetainedBodyRebindCandidate(command).kind =
                     "RebindRetainedBody"
    <2>1. /\ AsyncCandidateTyped(
                  RetainedBodyRebindCandidate(command))
           /\ RetainedBodyRebindCandidate(command).node = command.node
      BY <1>1, CausalCandidateFromTypedCommand
         DEF RetainedBodyRebindCandidate,
             AsyncCommandClasses, AsyncWorkKinds, AsyncReducerKinds
    <2>2. RetainedBodyRebindCandidate(command) \in AsyncCandidateSet
      BY <2>1, SMT DEF AsyncCandidateTyped, AsyncCandidateSet
    <2> QED BY <2>1, <2>2
       DEF RetainedBodyRebindCandidate, CausalCandidate,
           AsyncCandidateFrom, AsyncCandidateWithIdentity
  <1> QED BY <1>1

THEOREM DeliverProposalSchedulesRetainedBodyRebind ==
  \A command:
    command.kind = "DeliverProposal"
      => CommandSuccessors(command) =
           <<RetainedBodyRebindCandidate(command),
             CausalCandidate("Normal", "BeginPrepare", command)>>
BY DEF CommandSuccessors

THEOREM RebindSchedulesCurrentRoundStore ==
  \A command:
    command.kind = "RebindRetainedBody"
      => CommandSuccessors(command) =
           <<CausalCandidate("Completion", "StoreBody", command)>>
BY DEF CommandSuccessors

THEOREM StoreSchedulesCurrentRoundValidation ==
  \A command:
    command.kind = "StoreBody"
      => CommandSuccessors(command) =
           <<CausalCandidate("Completion", "ValidateBody", command)>>
BY DEF CommandSuccessors

THEOREM ValidationSchedulesPrepareAndLockedCommitAttempts ==
  \A command:
    command.kind = "ValidateBody"
      => CommandSuccessors(command) =
           <<CausalCandidate("Normal", "BeginPrepare", command),
             CausalCandidate("Completion", "BeginLockCommit", command),
             CausalCandidate("Completion", "Apply", command)>>
BY DEF CommandSuccessors

(***************************************************************************
The production adapter classifies `ValidationCompleted` as Completion, and
the reducer calls `persist_commit_intent` inside that event.  PrepareQC
processing likewise calls the same persistence routine directly when the
body is already validated.  The split Core commands therefore keep every
internal BeginLockCommit continuation in the Completion lane; treating one
as independent Progress could defer the exact persistence completion behind
an unrelated Progress-capacity fence.
***************************************************************************)
THEOREM PrepareQcDeliverySchedulesCompletionLockedCommitAttempt ==
  \A command:
    /\ command.kind = "DeliverQC"
    /\ command.item.envelope.qc.phase = "Prepare"
    => CommandSuccessors(command) =
         <<CausalCandidate("Progress", "BeginObservePrepare", command),
           CausalCandidate("Completion", "BeginLockCommit", command)>>
BY DEF CommandSuccessors

THEOREM PersistedPrepareObservationSchedulesCompletionLockedCommitAttempt ==
  \A command:
    command.kind = "PersistObservePrepare"
      => CommandSuccessors(command) =
           <<CausalCandidate("Completion", "BeginLockCommit", command)>>
BY DEF CommandSuccessors

THEOREM ReadyRetainedBodyRebindEnablesExecution ==
  \A command:
    RetainedBodyRebindReady(command)
      => ENABLED ExecuteCommand(command)
PROOF
  <1>1. ASSUME NEW command,
                RetainedBodyRebindReady(command)
         PROVE ENABLED ExecuteCommand(command)
    <2>1. PICK proposal \in SeenProposalValues:
             /\ CommandMatches(command, command.node, proposal.view,
                               proposal.subject)
             /\ ProposalAt(command.node, proposal) \in seenProposals
      BY <1>1 DEF RetainedBodyRebindReady
    <2>2. ENABLED RetainedBodyRebindAction(command, proposal)
      BY <1>1, <2>1, ExpandENABLED, Isa
         DEF RetainedBodyRebindReady, RetainedBodyRebindAction,
             CommandMatches, RebindRetainedBody, AsyncAuxVars
    <2>3. RetainedBodyRebindAction(command, proposal) \in BOOLEAN
      BY Isa DEF RetainedBodyRebindAction
    <2>4. ExecuteCommand(command) \in BOOLEAN
      BY Isa DEF ExecuteCommand
    <2>5. RetainedBodyRebindAction(command, proposal)
             => ExecuteCommand(command)
      BY Isa
         DEF RetainedBodyRebindAction, ExecuteCommand,
             ExecuteRegularCommand, RegularCoreCommand
    <2>6. (ENABLED RetainedBodyRebindAction(command, proposal))
             => ENABLED ExecuteCommand(command)
      BY <2>3, <2>4, <2>5, ENABLEDaxioms
    <2> QED BY <2>2, <2>6
  <1> QED BY <1>1

THEOREM ReadyRetainedBodyRebindIsDispatchable ==
  \A command:
    (RetainedBodyRebindReady(command)
      /\ command \in AsyncCandidateSet)
      => CommandDispatchable(command)
PROOF
  <1>1. ASSUME NEW command,
                RetainedBodyRebindReady(command),
                command \in AsyncCandidateSet
         PROVE \E selectedCommand \in AsyncCandidateSet:
                   /\ selectedCommand = command
                   /\ ENABLED ExecuteCommand(selectedCommand)
                   /\ (NodeIdle(selectedCommand.node)
                         \/ selectedCommand.class = "Completion")
    <2>1. ENABLED ExecuteCommand(command)
      BY <1>1, ReadyRetainedBodyRebindEnablesExecution
    <2>2. command.class = "Completion"
      BY <1>1 DEF RetainedBodyRebindReady
    <2>3. CandidateConsumerCurrent(command)
      BY <1>1 DEF RetainedBodyRebindReady
    <2>4. WITNESS command \in AsyncCandidateSet
    <2> QED BY <2>1, <2>2, <2>3
  <1> QED BY <1>1 DEF CommandDispatchable

THEOREM RebindCommandSelectsRetainedRebind ==
  \A command:
    (RegularCoreCommand(command) /\ command.kind = "RebindRetainedBody")
      => \E proposal \in SeenProposalValues:
           /\ CommandMatches(command, command.node, proposal.view,
                             proposal.subject)
           /\ RebindRetainedBody(command.node, proposal)
BY IsaT(60) DEF RegularCoreCommand

THEOREM ExecuteRebindStagesCurrentRoundBody ==
  \A command:
    (RegularCoreCommand(command) /\ command.kind = "RebindRetainedBody")
      => /\ BodyRecord(command.node, context', command.view,
                       command.subject)
                \in availableBodies'
         /\ RetainedLockedBodyHeldBy(
              retainedLockedBodies', command.node, context',
              command.subject)
PROOF
  <1>1. ASSUME NEW command,
                RegularCoreCommand(command),
                command.kind = "RebindRetainedBody"
         PROVE /\ BodyRecord(command.node, context', command.view,
                             command.subject)
                       \in availableBodies'
                /\ RetainedLockedBodyHeldBy(
                     retainedLockedBodies', command.node, context',
                     command.subject)
    <2>1. \E proposal \in SeenProposalValues:
             /\ CommandMatches(command, command.node, proposal.view,
                               proposal.subject)
             /\ RebindRetainedBody(command.node, proposal)
      BY <1>1, RebindCommandSelectsRetainedRebind
    <2>2. PICK proposal \in SeenProposalValues:
             /\ CommandMatches(command, command.node, proposal.view,
                               proposal.subject)
             /\ RebindRetainedBody(command.node, proposal)
      BY <2>1
    <2>3. /\ command.view = proposal.view
           /\ command.subject = proposal.subject
           /\ context' = context
           /\ retainedLockedBodies' = retainedLockedBodies
           /\ BodyRecord(command.node, context, proposal.view,
                         proposal.subject)
                \in availableBodies'
           /\ RetainedLockedBodyHeldBy(
                retainedLockedBodies, command.node, context,
                command.subject)
      BY <1>1, <2>2, Isa
         DEF CommandMatches, RebindRetainedBody, RegularCoreCommand
    <2> QED BY <2>3 DEF RetainedLockedBodyHeldBy
  <1> QED BY <1>1

THEOREM ValidationCommandSelectsValidationAction ==
  \A command:
    (RegularCoreCommand(command) /\ command.kind = "ValidateBody")
      => (\E proposal \in SeenProposalValues:
            /\ CommandMatches(command, command.node, proposal.view,
                              proposal.subject)
            /\ ValidateBody(command.node, proposal))
         \/ (\E proposal \in SeenProposalValues:
               /\ CommandMatches(command, command.node, proposal.view,
                                 proposal.subject)
               /\ RejectBody(command.node, proposal))
         \/ (\E qc \in DecisionQcValues:
               /\ CommandMatches(command, command.node, qc.view, qc.subject)
               /\ ValidateDecidedBody(command.node, qc))
         \/ (\E qc \in prepareQCs:
               /\ CommandMatches(command, command.node, qc.view, qc.subject)
               /\ ValidateLockedBody(command.node, qc))
BY Isa DEF RegularCoreCommand

THEOREM ExecuteValidationBindsCurrentViewAndGeneration ==
  \A command:
    (RegularCoreCommand(command) /\ command.kind = "ValidateBody")
      => \/ BodyValidatedBy(
               validatedBodies', command.node, context', command.view,
               generation'[command.node], command.subject)
         \/ BodyRecord(command.node, context', command.view,
                       command.subject)
               \in invalidBodies'
PROOF
  <1>1. ASSUME NEW command,
                RegularCoreCommand(command),
                command.kind = "ValidateBody"
         PROVE \/ BodyValidatedBy(
                      validatedBodies', command.node, context', command.view,
                      generation'[command.node], command.subject)
                \/ BodyRecord(command.node, context', command.view,
                              command.subject)
                      \in invalidBodies'
    <2>1. (\E proposal \in SeenProposalValues:
              /\ CommandMatches(command, command.node, proposal.view,
                                proposal.subject)
              /\ ValidateBody(command.node, proposal))
           \/ (\E proposal \in SeenProposalValues:
                 /\ CommandMatches(command, command.node, proposal.view,
                                   proposal.subject)
                 /\ RejectBody(command.node, proposal))
           \/ (\E qc \in DecisionQcValues:
                 /\ CommandMatches(command, command.node, qc.view,
                                   qc.subject)
                 /\ ValidateDecidedBody(command.node, qc))
           \/ (\E qc \in prepareQCs:
                 /\ CommandMatches(command, command.node, qc.view,
                                   qc.subject)
                 /\ ValidateLockedBody(command.node, qc))
      BY <1>1, ValidationCommandSelectsValidationAction
    <2>2. CASE \E proposal \in SeenProposalValues:
                    /\ CommandMatches(
                         command, command.node, proposal.view,
                         proposal.subject)
                    /\ ValidateBody(command.node, proposal)
      <3>1. PICK proposal \in SeenProposalValues:
               /\ CommandMatches(command, command.node, proposal.view,
                                 proposal.subject)
               /\ ValidateBody(command.node, proposal)
        BY <2>2
      <3> QED BY <3>1, Isa
           DEF CommandMatches, ValidateBody, BodyValidatedBy
    <2>3. CASE \E proposal \in SeenProposalValues:
                    /\ CommandMatches(
                         command, command.node, proposal.view,
                         proposal.subject)
                    /\ RejectBody(command.node, proposal)
      <3>1. PICK proposal \in SeenProposalValues:
               /\ CommandMatches(command, command.node, proposal.view,
                                 proposal.subject)
               /\ RejectBody(command.node, proposal)
        BY <2>3
      <3> QED BY <3>1, Isa
           DEF CommandMatches, RejectBody, BodyRecord
    <2>4. CASE \E qc \in DecisionQcValues:
                    /\ CommandMatches(
                         command, command.node, qc.view, qc.subject)
                    /\ ValidateDecidedBody(command.node, qc)
      <3>1. PICK qc \in DecisionQcValues:
               /\ CommandMatches(command, command.node, qc.view, qc.subject)
               /\ ValidateDecidedBody(command.node, qc)
        BY <2>4
      <3> QED BY <3>1, Isa
           DEF CommandMatches, ValidateDecidedBody, BodyValidatedBy
    <2>5. CASE \E qc \in prepareQCs:
                    /\ CommandMatches(
                         command, command.node, qc.view, qc.subject)
                    /\ ValidateLockedBody(command.node, qc)
      <3>1. PICK qc \in prepareQCs:
               /\ CommandMatches(command, command.node, qc.view, qc.subject)
               /\ ValidateLockedBody(command.node, qc)
        BY <2>5
      <3> QED BY <3>1, Isa
           DEF CommandMatches, ValidateLockedBody, BodyValidatedBy
    <2> QED BY <2>1, <2>2, <2>3, <2>4, <2>5
  <1> QED BY <1>1

(***************************************************************************
Locked-round CommitVote recovery after a TC install.  Prepare admission remains
current-view-only.  The install clears only the installing node's volatile
vote receipts.  Retained CommitVote control is still retryable, and every
Commit delivery or locally formed CommitQC requires the exact durable Prepare
lock.  Persisting a replacement lock retires the superseded historical pool
while preserving current-view work and the new exact locked Commit pool.
***************************************************************************)

THEOREM PrepareVoteAdmissionIsCurrentView ==
  \A node, vote:
    (vote.phase = "Prepare" /\ VoteRoundAdmissible(node, vote))
      => vote.view = nodeView[node]
BY DEF VoteRoundAdmissible

THEOREM CommitVoteAdmissionIsExactLockedCommit ==
  \A node, vote:
    (vote.phase = "Commit" /\ VoteRoundAdmissible(node, vote))
      => LockedPrepareRound(node, vote.view, vote.subject)
BY DEF VoteRoundAdmissible

THEOREM CommitFormationIsExactLockedRound ==
  \A node, roundView, subject:
    CommitRoundAdmissible(node, roundView, subject)
      => LockedPrepareRound(node, roundView, subject)
BY DEF CommitRoundAdmissible

THEOREM HistoricalVoteAdmissionIsExactLockedCommit ==
  \A node, vote:
    (VoteRoundAdmissible(node, vote)
      /\ vote.view # nodeView[node])
      => /\ vote.phase = "Commit"
         /\ LockedPrepareRound(node, vote.view, vote.subject)
BY CommitVoteAdmissionIsExactLockedCommit

THEOREM HistoricalCommitFormationIsExactLockedRound ==
  \A node, roundView, subject:
    (CommitRoundAdmissible(node, roundView, subject)
      /\ roundView # nodeView[node])
      => LockedPrepareRound(node, roundView, subject)
BY CommitFormationIsExactLockedRound

THEOREM HistoricalLockedCommitUsesProgressReserve ==
  \A item:
    HistoricalLockedCommitItem(item)
      => DeliveryClass(item) = "Progress"
BY DEF DeliveryClass

(***************************************************************************
Executing a scheduled historical BeginLockCommit may select a different
valid Prepare QcRecord than the candidate's concrete evidence when both
records have the same production CertificateRef.  The action persists the
selected exact record, while progress ownership transfers by the stable
Prepare reference.  StrongInductiveInvariant supplies the redundant
`height = context.height` fact for both authenticated QCs; coordinate matching
alone would not establish the full reference over the broad QcRecord carrier.
***************************************************************************)
THEOREM HistoricalBeginLockExecutionCreatesSameRefPending ==
  \A node \in ValidatorIds, sourceQc \in QcRecordSet,
     command \in AsyncCandidateSet:
    /\ StrongInductiveInvariant
    /\ HistoricalLockedPrepareForCommit(node, sourceQc)
    /\ HistoricalBeginLockRecoveryCandidate(node, sourceQc, command)
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
                HistoricalBeginLockRecoveryCandidate(
                  node, sourceQc, command),
                ExecuteCommand(command)
         PROVE \E request \in pendingLockCommit':
                 /\ request.node = node
                 /\ SamePrepareRecoveryRef(request.qc, sourceQc)
    <2>1. PICK selectedQc \in LockCommitQcValues:
             /\ CommandMatches(command, command.node,
                               selectedQc.view, selectedQc.subject)
             /\ BeginLockCommit(command.node, selectedQc)
      BY <1>1, IsaT(60)
         DEF HistoricalBeginLockRecoveryCandidate,
             ExecuteCommand, ExecuteRegularCommand, RegularCoreCommand
    <2>2. /\ command.node = node
           /\ command.view = sourceQc.view
           /\ command.subject = sourceQc.subject
      BY <1>1 DEF HistoricalBeginLockRecoveryCandidate
    <2>3. /\ selectedQc.context = context
           /\ selectedQc.phase = "Prepare"
           /\ pendingLockCommit' =
                pendingLockCommit
                  \cup {LockCommitWal(
                          command.node, selectedQc,
                          Vote(context, selectedQc.view, "Commit",
                               selectedQc.subject, command.node))}
      BY <2>1 DEF BeginLockCommit
    <2>4. selectedQc \in prepareQCs
      BY <1>1, <2>1, IsaT(90)
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             LineageInvariant, QcTransportBacked,
             CertificatePhasesCorrect, LockCommitQcValues,
             ReceivedQcValues, CurrentOpenPrepareForCommit,
             HistoricalLockedPrepareForCommit,
             HistoricalLockedPrepareSource, LockedPrepareRecoverySource,
             BeginLockCommit
    <2>5. /\ sourceQc.context = context
           /\ sourceQc \in prepareQCs
           /\ sourceQc.height = sourceQc.context.height
           /\ selectedQc.height = selectedQc.context.height
           /\ selectedQc \in QcRecordSet
      BY <1>1, <2>4, IsaT(90)
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             ReducerProvenanceInvariant, CertificatesBackedByIntents,
             HistoricalQcValid, HistoricalLockedPrepareForCommit,
             HistoricalLockedPrepareSource, LockedPrepareRecoverySource
    <2>6. SamePrepareRecoveryRef(selectedQc, sourceQc)
      BY <1>1, <2>1, <2>2, <2>3, <2>5, SMT
         DEF CommandMatches, SamePrepareRecoveryRef,
             SameCertificateRef, CertificateRefOf
    <2> DEFINE SelectedVote ==
           Vote(context, selectedQc.view, "Commit",
                selectedQc.subject, command.node)
    <2> DEFINE SelectedRequest ==
           LockCommitWal(command.node, selectedQc, SelectedVote)
    <2>7. /\ SelectedRequest \in pendingLockCommit'
           /\ SelectedRequest.node = node
           /\ SamePrepareRecoveryRef(SelectedRequest.qc, sourceQc)
      BY <2>2, <2>3, <2>6, Isa
         DEF SelectedRequest, SelectedVote, LockCommitWal
    <2> QED BY <2>7
  <1> QED BY <1>1

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
       SerializedRuntimeStep, RuntimeStep,
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
