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

Replay tracks the exact ingress-admission owner read by `RunNodeWork`.
Restart independently revalidates every unsealed active-height Serve
lifecycle and persists its typed terminal before replay becomes visible.
Consequently replay observes neither a physical Serve carrier nor
requester-dependent suspended Serve debt; the monotone lifecycle and
scheduler high-watermarks remain retained.  This is separate from the
durable leader-wire and Candidate-continuation Dormant states.
***************************************************************************)

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

(***************************************************************************
The activation/deadline correspondence is relational scheduler safety, not a
raw inner-action type fact.  Inner scheduler actions deliberately leave the
appended activation field open; the complete `AsyncNext` relation closes it
with `AsyncServiceActivationTransition`.  Keep this induction at that exact
boundary so a raw action theorem cannot assume the correspondence it is meant
to preserve.
***************************************************************************)
THEOREM AsyncInitEstablishesServiceActivationPairInvariant ==
  \A initialContext:
    AsyncInitAt(initialContext)
      => AsyncServiceActivationPairInvariant
BY Isa
   DEF AsyncInitAt, AsyncBaseInitAt, AsyncTransportInit,
       AsyncServiceActivationPairInvariant,
       AsyncServiceActivationStateSet,
       AsyncActiveServiceNodes

THEOREM AsyncNextPreservesServiceActivationPairInvariant ==
  /\ AsyncTypeInvariant
  /\ AsyncNext
  => AsyncServiceActivationPairInvariant'
BY IsaT(1200)
   DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
       AsyncConfiguration,
       AsyncServiceActivationPairInvariant,
       AsyncServiceActivationStateSet,
       AsyncServiceActivationRestricted,
       AsyncActiveServiceNodes,
       AsyncNext, AsyncServiceActivationTransition,
       AsyncEnterIndexedServiceActivation,
       AsyncActivateServiceNode,
       AsyncServiceActivationFrameVars,
       AsyncSchedulerExceptServiceActivation,
       AsyncNonCrashStep, AsyncRunnerStep,
       RunNode, RunHistoricalRecoveryNode, RunNodeWork,
       ResolveRunNodeCandidateProducerContinuation,
       ReplayRunNodeCandidateProducerContinuation,
       AsyncCandidateProducerContinuationExactLocalReplayStep,
       AsyncCandidateProducerContinuationReplayTargetOnlyTurn,
       AsyncCandidateProducerContinuationExactRuntimeReplayStep,
       AsyncSchedulerExceptCausalControlAndNodeService,
       AsyncSchedulerExceptCausalControlCommandRunnerAndNodeService,
       AsyncSchedulerExceptCausalControlRunnerAndNodeService,
       RunHistoricalServer, HistoricalIdleStep,
       AsyncNonRunnerStep, AsyncSetGST, AsyncTick,
       AsyncNonClockVars, OpenHistoricalRecovery,
       DirectCommitCertificateDiscoveryStep,
       DirectHistoricalCommitCertificateDiscoveryStep,
       CommitCertificateDiscoveryStepWork,
       ServiceIoWorker, ServiceHistoricalRecoveryIoWorker,
       ServiceIoWorkerWork,
       EnqueueIoLocalControl,
       EnqueueHistoricalRecoveryIoLocalControl,
       EnqueueIoLocalControlWork,
       AsyncNetworkStep, AdmitIngressPacket,
       AdmitHiddenPacket, CoalesceHiddenPacket,
       DropPolicyRejectedHiddenPacket,
       AsyncFaultStep, PreGstLosePacket,
       PreGstServeReceiverCloseRollback, PreGstCrash,
       InjectByzantineNoise,
       InjectUntrustedTransportCompletion,
       InjectAuthenticatedJunk,
       InjectByzantineCertifiedRequest,
       AsyncByzantineProposal, AsyncByzantineVote,
       AsyncByzantineTimeout,
       DriveResponsiveReplayHead, FinishResponsiveReplay,
       RearmResponsiveRecovery,
       PreGstResponsiveCrash, PreGstResponsiveRestart,
       PreGstResponsiveReplay, ResetNodeSchedulerForRestart,
       AsyncNonCrashOuterFrame, AsyncNonRunnerOuterFrame,
       AsyncRecoveryOuterFrame, AsyncSchedulerVars,
       AsyncRecoveryControlVars, AsyncRecoveryVars,
       AsyncIoVars, AsyncDeferredVars,
       AsyncLocalAdmissionVars, vars

THEOREM AsyncInitEstablishesLeaderWireIngressCarrierOwnership ==
  \A initialContext:
    AsyncInitAt(initialContext)
      => AsyncLeaderWireIngressCarrierOwnershipInvariant
BY FS_CardinalityType, Isa
   DEF AsyncInitAt, AsyncBaseInitAt, AsyncTransportInit,
       AsyncLeaderWireIngressCarrierOwnershipInvariant,
       AsyncLeaderWireIngressCarrierCoordinates,
       AsyncLeaderWireLifecycleIngressProtected,
       AsyncLeaderWireAdmissionMatchesRecord,
       AsyncLeaderWireLifecycleIdentityDerivable,
       AsyncChunkExactLifecycleCoordinatesRetained,
       IngressLane, IngressLaneDepth

THEOREM AsyncInitEstablishesOrdinaryIngressCarrierOwnership ==
  \A initialContext:
    AsyncInitAt(initialContext)
      => AsyncOrdinaryIngressCarrierOwnershipInvariant
BY FS_CardinalityType, Isa
   DEF AsyncInitAt, AsyncBaseInitAt, AsyncTransportInit,
       AsyncOrdinaryIngressCarrierOwnershipInvariant

THEOREM AsyncProducerPacketRouteProjectsToIngressCarrier ==
  \A packet:
    AsyncPacketTyped(packet)
      => AsyncProducerIngressRequest(packet.item)
           \in AsyncProducerIngressRequests
PROOF
  <1>1. ASSUME NEW packet, AsyncPacketTyped(packet)
         PROVE AsyncProducerIngressRequest(packet.item)
                 \in AsyncProducerIngressRequests
    <2>1. PICK carrier \in AsyncNetworkItems:
             packet.transportIdentity =
               AsyncTransportRouteIdentity(carrier)
      BY <1>1, Isa
         DEF AsyncPacketTyped, AsyncTransportRouteIdentitySet
    <2>2. AsyncTransportRouteIdentity(packet.item) =
             AsyncTransportRouteIdentity(carrier)
      BY <1>1, <2>1 DEF AsyncPacketTyped
    <2>3. packet.item.kind = "CertifiedResponse"
             => carrier.kind = "CertifiedResponse"
      <3>1. SUFFICES ASSUME
               packet.item.kind = "CertifiedResponse",
               carrier.kind # "CertifiedResponse"
             PROVE FALSE
        BY Zenon
      <3>2. DOMAIN AsyncTransportRouteIdentity(packet.item) =
               {"kind", "envelope"}
        BY <3>1, Isa
           DEF AsyncTransportRouteIdentity,
               AsyncLeaderWireServiceIdentity
      <3>3. DOMAIN AsyncTransportRouteIdentity(carrier) =
               {"kind", "source", "envelope"}
        BY <3>1, Isa
           DEF AsyncTransportRouteIdentity,
               AsyncLeaderWireServiceIdentity
      <3> QED BY <2>2, <3>2, <3>3, Isa
    <2>4. carrier.kind = "CertifiedResponse"
             => packet.item.kind = "CertifiedResponse"
      <3>1. SUFFICES ASSUME
               carrier.kind = "CertifiedResponse",
               packet.item.kind # "CertifiedResponse"
             PROVE FALSE
        BY Zenon
      <3>2. DOMAIN AsyncTransportRouteIdentity(carrier) =
               {"kind", "envelope"}
        BY <3>1, Isa
           DEF AsyncTransportRouteIdentity,
               AsyncLeaderWireServiceIdentity
      <3>3. DOMAIN AsyncTransportRouteIdentity(packet.item) =
               {"kind", "source", "envelope"}
        BY <3>1, Isa
           DEF AsyncTransportRouteIdentity,
               AsyncLeaderWireServiceIdentity
      <3> QED BY <2>2, <3>2, <3>3, Isa
    <2>5. packet.item.kind = "CertifiedResponse"
             <=> carrier.kind = "CertifiedResponse"
      BY <2>3, <2>4, Zenon
    <2>6. CASE packet.item.kind = "CertifiedResponse"
      <3>1. carrier.kind = "CertifiedResponse"
        BY <2>5, <2>6, Zenon
      <3>2. packet.item.envelope = carrier.envelope
        BY <2>2, <2>6, <3>1, Isa
           DEF AsyncTransportRouteIdentity,
               AsyncLeaderWireServiceIdentity
      <3>3. AsyncProducerIngressRequest(packet.item) =
               AsyncProducerIngressRequest(carrier)
        BY <2>6, <3>1, <3>2, Isa
           DEF AsyncProducerIngressRequest, AsyncReplyRequestKinds,
               AsyncCertifiedResponseCanonicalWireIdentity
      <3> QED BY <2>1, <3>3, Isa
           DEF AsyncProducerIngressRequests
    <2>7. CASE packet.item.kind # "CertifiedResponse"
      <3>1. carrier.kind # "CertifiedResponse"
        BY <2>5, <2>7, Zenon
      <3>2. /\ packet.item.kind = carrier.kind
             /\ packet.item.source = carrier.source
             /\ packet.item.envelope = carrier.envelope
        BY <2>2, <2>7, <3>1, Isa
           DEF AsyncTransportRouteIdentity,
               AsyncLeaderWireServiceIdentity
      <3>3. CASE packet.item.kind \in AsyncReplyRequestKinds
        <4>1. AsyncProducerIngressRequest(packet.item) =
                 AsyncProducerIngressRequest(carrier)
          BY <3>2, <3>3, Isa
             DEF AsyncProducerIngressRequest,
                 AsyncServeLogicalRequestIdentity,
                 AsyncReplySemanticIdentity
        <4> QED BY <2>1, <4>1, Isa
             DEF AsyncProducerIngressRequests
      <3>4. CASE packet.item.kind \notin AsyncReplyRequestKinds
        <4>1. AsyncProducerIngressRequest(packet.item) =
                 AsyncProducerIngressRequest(carrier)
          BY <2>7, <3>1, <3>2, <3>4, Isa
             DEF AsyncProducerIngressRequest, AsyncNetworkItem
        <4> QED BY <2>1, <4>1, Isa
             DEF AsyncProducerIngressRequests
      <3> QED BY <3>3, <3>4
    <2> QED BY <2>6, <2>7
  <1> QED BY <1>1

THEOREM AdmitIngressPacketHasDueSourcePacket ==
  \A recipient, source:
    AdmitIngressPacket(recipient, source)
      => DueSourcePackets(recipient, source) # {}
BY Isa
   DEF AdmitIngressPacket, AdmitHiddenPacket, CoalesceHiddenPacket,
       DropPolicyRejectedHiddenPacket,
       DropExactActiveLeaderWireRetry

THEOREM AsyncProducerAdmittedIngressProjectionIsFinite ==
  AsyncTypeInvariant
    => /\ IsFiniteSet(AsyncProducerAdmittedIngressEpisodes)
       /\ IsFiniteSet(AsyncProducerAdmittedIngressObligations)
       /\ IsFiniteSet(AsyncProducerAdmittedIngressOrigins)
PROOF
  <1>1. ASSUME AsyncTypeInvariant
         PROVE /\ IsFiniteSet(AsyncProducerAdmittedIngressEpisodes)
               /\ IsFiniteSet(AsyncProducerAdmittedIngressObligations)
               /\ IsFiniteSet(AsyncProducerAdmittedIngressOrigins)
    <2>1. IsFiniteSet(AsyncIngressSources)
      BY <1>1, AsyncIngressSourcesAreFinite
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
             TypeInvariant
    <2>2. IsFiniteSet(ValidatorIds)
      BY <1>1, RuntimeValidatorIdsAreFinite DEF AsyncTypeInvariant
    <2>3. IsFiniteSet(AsyncProducerAdmittedIngressCoordinates)
      BY <2>1, <2>2, FS_Product, FS_Subset, Zenon
         DEF AsyncProducerAdmittedIngressCoordinates
    <2>4. IsFiniteSet(AsyncProducerAdmittedIngressEpisodes)
      BY <2>3, FS_Image
         DEF AsyncProducerAdmittedIngressEpisodes
    <2>5. IsFiniteSet(AsyncProducerAdmittedIngressObligations)
      BY <2>4, FS_Image
         DEF AsyncProducerAdmittedIngressObligations
    <2>6. IsFiniteSet(AsyncProducerAdmittedIngressOrigins)
      BY <2>4, FS_Image
         DEF AsyncProducerAdmittedIngressOrigins
    <2> QED BY <2>4, <2>5, <2>6
  <1> QED BY <1>1

THEOREM AsyncProducerAdmittedIngressProjectionIsTyped ==
  AsyncTypeInvariant
    => /\ AsyncProducerAdmittedIngressEpisodes
              \subseteq AsyncProducerIngressEpisodeSet
       /\ AsyncProducerAdmittedIngressObligations
              \subseteq AsyncProducerObligationSet
       /\ AsyncProducerAdmittedIngressOrigins
              \subseteq AsyncProducerIngressOriginSet
PROOF
  <1>1. ASSUME AsyncTypeInvariant
         PROVE /\ AsyncProducerAdmittedIngressEpisodes
                      \subseteq AsyncProducerIngressEpisodeSet
               /\ AsyncProducerAdmittedIngressObligations
                      \subseteq AsyncProducerObligationSet
               /\ AsyncProducerAdmittedIngressOrigins
                      \subseteq AsyncProducerIngressOriginSet
    <2>1. AsyncPacketContentTypeInvariant
      BY <1>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncTransportTypeInvariant,
             AsyncTransportContentTypeInvariant
    <2>2. \A coordinate \in AsyncProducerAdmittedIngressCoordinates:
             /\ coordinate[1] \in ValidatorIds
             /\ coordinate[2] \in AsyncIngressSources
             /\ DueSourcePackets(coordinate[1], coordinate[2]) # {}
      BY AdmitIngressPacketHasDueSourcePacket, Isa
         DEF AsyncProducerAdmittedIngressCoordinates
    <2>3. \A coordinate \in AsyncProducerAdmittedIngressCoordinates:
             AsyncPacketTyped(
               OldestDueSourcePacket(coordinate[1], coordinate[2]))
      BY <2>1, <2>2, OldestDueSourcePacketFacts, Isa
    <2>4. \A coordinate \in AsyncProducerAdmittedIngressCoordinates:
             AsyncProducerIngressRequest(
               OldestDueSourcePacket(coordinate[1], coordinate[2]).item)
               \in AsyncProducerIngressRequests
      BY <2>3, AsyncProducerPacketRouteProjectsToIngressCarrier, Zenon
    <2>5. AsyncProducerAdmittedIngressEpisodes
             \subseteq AsyncProducerIngressEpisodeSet
      <3>1. SUFFICES ASSUME
               NEW episode \in AsyncProducerAdmittedIngressEpisodes
             PROVE episode \in AsyncProducerIngressEpisodeSet
        BY Zenon
      <3>2. PICK coordinate
                    \in AsyncProducerAdmittedIngressCoordinates:
               episode =
                 AsyncProducerIngressEpisode(
                   OldestDueSourcePacket(
                     coordinate[1], coordinate[2]).item,
                   OldestDueSourcePacket(
                     coordinate[1], coordinate[2]).authenticatedSource)
        BY <3>1, Zenon
           DEF AsyncProducerAdmittedIngressEpisodes
      <3> DEFINE Packet ==
             OldestDueSourcePacket(coordinate[1], coordinate[2])
      <3>3. AsyncPacketTyped(Packet)
        BY <2>3, <3>2
      <3>4. AsyncProducerIngressRequest(Packet.item)
               \in AsyncProducerIngressRequests
        BY <2>4, <3>2
      <3>5. Packet.authenticatedSource \in AsyncIngressSources
        BY <3>3, Isa DEF AsyncPacketTyped
      <3> QED BY <3>2, <3>4, <3>5, Isa
           DEF Packet, AsyncProducerIngressEpisodeSet,
               AsyncProducerIngressEpisode, AsyncProducerEpisode
    <2>6. AsyncProducerAdmittedIngressObligations
             \subseteq AsyncProducerObligationSet
      <3>1. SUFFICES ASSUME
               NEW obligation \in AsyncProducerAdmittedIngressObligations
             PROVE obligation \in AsyncProducerObligationSet
        BY Zenon
      <3>2. PICK episode \in AsyncProducerAdmittedIngressEpisodes:
               obligation = AsyncProducerEpisodeObligation(episode)
        BY <3>1, Zenon
           DEF AsyncProducerAdmittedIngressObligations
      <3>3. episode \in AsyncProducerIngressEpisodeSet
        BY <2>5, <3>2
      <3>4. /\ episode.request \in AsyncProducerIngressRequests
             /\ episode.authenticatedSource \in AsyncIngressSources
        BY <3>3, Isa
           DEF AsyncProducerIngressEpisodeSet
      <3> QED BY <3>2, <3>4, Isa
           DEF AsyncProducerEpisodeObligation,
               AsyncProducerObligation, AsyncProducerObligationSet
    <2>7. AsyncProducerAdmittedIngressOrigins
             \subseteq AsyncProducerIngressOriginSet
      <3>1. SUFFICES ASSUME
               NEW origin \in AsyncProducerAdmittedIngressOrigins
             PROVE origin \in AsyncProducerIngressOriginSet
        BY Zenon
      <3>2. PICK episode \in AsyncProducerAdmittedIngressEpisodes:
               origin = AsyncProducerCanonicalOrigin(episode)
        BY <3>1, Zenon
           DEF AsyncProducerAdmittedIngressOrigins
      <3>3. episode \in AsyncProducerIngressEpisodeSet
        BY <2>5, <3>2
      <3>4. episode.request \in AsyncProducerIngressRequests
        BY <3>3, Isa
           DEF AsyncProducerIngressEpisodeSet
      <3>5. [kind |-> "Ingress", request |-> episode.request]
                 \in AsyncProducerIngressOwnerSet
        BY <3>4, Isa
           DEF AsyncProducerIngressOwnerSet
      <3> QED BY <3>2, <3>3, <3>5, Isa
           DEF AsyncProducerCanonicalOrigin, AsyncProducerOrigin,
               AsyncProducerIngressOriginSet
    <2> QED BY <2>5, <2>6, <2>7
  <1> QED BY <1>1

THEOREM AsyncProducerProjectionPreservesTypeInvariant ==
  /\ AsyncTypeInvariant
  /\ AsyncProducerProjectionStep
  => AsyncProducerTypeInvariant'
PROOF
  <1>1. ASSUME /\ AsyncTypeInvariant
                /\ AsyncProducerProjectionStep
         PROVE AsyncProducerTypeInvariantAt(
                 asyncProducerKnownObligations',
                 asyncProducerConsumedEpisodes',
                 asyncProducerOriginHistory')
    <2>1. /\ AsyncProducerTypeInvariantAt(
                 asyncProducerKnownObligations,
                 asyncProducerConsumedEpisodes,
                 asyncProducerOriginHistory)
           /\ AsyncProducerProjectionStep
      BY <1>1
         DEF AsyncTypeInvariant, AsyncProducerTypeInvariant
    <2>2. /\ AsyncProducerJournalClosedAt(
                  asyncProducerKnownObligations,
                  asyncProducerConsumedEpisodes,
                  asyncProducerOriginHistory)
           /\ \A origin \in asyncProducerOriginHistory:
                /\ origin.producerEpisode
                     \in asyncProducerConsumedEpisodes
                /\ origin.owner.request = origin.producerEpisode.request
      BY <2>1 DEF AsyncProducerTypeInvariantAt
    <2>3. /\ IsFiniteSet(AsyncProducerAdmittedIngressEpisodes)
           /\ IsFiniteSet(AsyncProducerAdmittedIngressObligations)
           /\ IsFiniteSet(AsyncProducerAdmittedIngressOrigins)
      BY <1>1, AsyncProducerAdmittedIngressProjectionIsFinite
    <2>4. /\ AsyncProducerAdmittedIngressEpisodes
                  \subseteq AsyncProducerIngressEpisodeSet
           /\ AsyncProducerAdmittedIngressObligations
                  \subseteq AsyncProducerObligationSet
           /\ AsyncProducerAdmittedIngressOrigins
                  \subseteq AsyncProducerIngressOriginSet
      BY <1>1, AsyncProducerAdmittedIngressProjectionIsTyped
    <2>5. /\ IsFiniteSet(asyncProducerKnownObligations')
           /\ IsFiniteSet(asyncProducerConsumedEpisodes')
           /\ IsFiniteSet(asyncProducerOriginHistory')
      BY <2>1, <2>3, FS_Union, Zenon
         DEF AsyncProducerTypeInvariantAt, AsyncProducerProjectionStep
    <2>6. /\ asyncProducerKnownObligations'
                  \subseteq AsyncProducerObligationSet
           /\ asyncProducerConsumedEpisodes'
                  \subseteq AsyncProducerIngressEpisodeSet
           /\ asyncProducerOriginHistory'
                  \subseteq AsyncProducerIngressOriginSet
      BY <2>1, <2>4, Isa
         DEF AsyncProducerTypeInvariantAt, AsyncProducerProjectionStep
    <2>7. \A episode \in AsyncProducerAdmittedIngressEpisodes:
             /\ AsyncProducerEpisodeObligation(episode)
                  \in AsyncProducerAdmittedIngressObligations
             /\ AsyncProducerCanonicalOrigin(episode)
                  \in AsyncProducerAdmittedIngressOrigins
      BY Isa
         DEF AsyncProducerAdmittedIngressObligations,
             AsyncProducerAdmittedIngressOrigins
    <2>8. AsyncProducerJournalClosedAt(
             asyncProducerKnownObligations',
             asyncProducerConsumedEpisodes',
             asyncProducerOriginHistory')
      <3>1. asyncProducerConsumedEpisodes'
                 \subseteq AsyncProducerIngressEpisodeSet
        BY <2>6
      <3>2. \A episode \in asyncProducerConsumedEpisodes':
               /\ AsyncProducerEpisodeObligation(episode)
                    \in asyncProducerKnownObligations'
               /\ AsyncProducerCanonicalOrigin(episode)
                    \in asyncProducerOriginHistory'
        <4>1. ASSUME NEW episode
                           \in asyncProducerConsumedEpisodes'
               PROVE /\ AsyncProducerEpisodeObligation(episode)
                            \in asyncProducerKnownObligations'
                     /\ AsyncProducerCanonicalOrigin(episode)
                            \in asyncProducerOriginHistory'
          <5>1. episode \in asyncProducerConsumedEpisodes
                   \/ episode \in AsyncProducerAdmittedIngressEpisodes
            BY <2>1, <4>1, Isa DEF AsyncProducerProjectionStep
          <5>2. CASE episode \in asyncProducerConsumedEpisodes
            <6> QED BY <2>1, <2>2, <5>2, Isa
                 DEF AsyncProducerProjectionStep,
                     AsyncProducerJournalClosedAt
          <5>3. CASE episode \in AsyncProducerAdmittedIngressEpisodes
            <6> QED BY <2>1, <2>7, <5>3, Isa
                 DEF AsyncProducerProjectionStep
          <5> QED BY <5>1, <5>2, <5>3
        <4> QED BY <4>1
      <3> QED BY <3>1, <3>2, Zenon
           DEF AsyncProducerJournalClosedAt
    <2>9. \A origin \in AsyncProducerAdmittedIngressOrigins:
             /\ origin.producerEpisode
                  \in AsyncProducerAdmittedIngressEpisodes
             /\ origin.owner.request = origin.producerEpisode.request
      <3>1. ASSUME NEW origin \in AsyncProducerAdmittedIngressOrigins
             PROVE /\ origin.producerEpisode
                          \in AsyncProducerAdmittedIngressEpisodes
                   /\ origin.owner.request = origin.producerEpisode.request
        <4>1. PICK episode \in AsyncProducerAdmittedIngressEpisodes:
                 origin = AsyncProducerCanonicalOrigin(episode)
          BY <3>1, Zenon DEF AsyncProducerAdmittedIngressOrigins
        <4> QED BY <4>1, Isa
             DEF AsyncProducerCanonicalOrigin, AsyncProducerOrigin
      <3> QED BY <3>1
    <2>10. \A origin \in asyncProducerOriginHistory':
              /\ origin.producerEpisode
                   \in asyncProducerConsumedEpisodes'
              /\ origin.owner.request = origin.producerEpisode.request
      BY <2>1, <2>2, <2>9, Isa DEF AsyncProducerProjectionStep
    <2> QED BY <2>5, <2>6, <2>8, <2>10, Zenon
         DEF AsyncProducerTypeInvariantAt,
             AsyncProducerJournalClosedAt
  <1> QED BY <1>1 DEF AsyncProducerTypeInvariant

THEOREM AsyncProducerVarsFramePreservesTypeInvariant ==
  /\ AsyncProducerTypeInvariant
  /\ UNCHANGED AsyncProducerVars
  => AsyncProducerTypeInvariant'
BY Zenon
   DEF AsyncProducerTypeInvariant, AsyncProducerTypeInvariantAt,
       AsyncProducerJournalClosed, AsyncProducerJournalClosedAt,
       AsyncProducerVars

THEOREM AsyncServeProducerTurnFramePreservesTypeInvariant ==
  /\ AsyncServeProducerTurnTypeInvariant
  /\ UNCHANGED asyncServeProducerTurnReady
  => AsyncServeProducerTurnTypeInvariant'
BY Zenon DEF AsyncServeProducerTurnTypeInvariant


THEOREM AsyncStrongTypeProjectsAsyncType ==
  AsyncStrongTypeInvariant => AsyncTypeInvariant
BY DEF AsyncStrongTypeInvariant, AsyncTypeInvariant,
       StrongInductiveInvariant, Safety

AsyncTimeoutRecoveryEpisodeBoundaryIn(
    episode, currentContext, currentNodeView,
    currentGeneration, currentDecisions) ==
  /\ episode.key.context = currentContext
  /\ episode.timeoutOwnerOrigin.height = currentContext.height
  /\ episode.key.view = currentNodeView[episode.node]
  /\ episode.generation = currentGeneration[episode.node]
  /\ ~AsyncNodeHasDecisionIn(
       episode.node, currentContext, currentDecisions)

THEOREM AsyncInitEstablishesTimeoutRecoveryCurrentBoundary ==
  \A initialContext:
    AsyncInitAt(initialContext)
      => AsyncTimeoutRecoveryEpisodeCurrentBoundaryInvariant
BY AsyncTimeoutRecoveryRolloverInstanceStartsEmpty, Isa
   DEF AsyncInitAt, AsyncBaseInitAt,
       AsyncTimeoutRecoveryEpisodeCurrentBoundaryInvariant,
       AsyncTimeoutRecoveryEpisodes,
       AsyncTimeoutRecoveryEpisodesIn

THEOREM AsyncInitEstablishesServeProducerTurnInvariants ==
  \A initialContext:
    AsyncInitAt(initialContext)
      => /\ AsyncServeProducerTurnTypeInvariant
         /\ AsyncServeProducerTurnOwnershipInvariant
BY Isa
   DEF AsyncInitAt, AsyncBaseInitAt,
       AsyncServeProducerTurnInit,
       AsyncServeProducerTurnTypeInvariant,
       AsyncServeProducerTurnOwnershipInvariant

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
    <2>3c. AsyncServiceActivationPairInvariant
      BY <1>1, AsyncInitEstablishesServiceActivationPairInvariant
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
             AsyncCandidateServiceTombstoneSet,
             AsyncCandidateProducerContinuationLifecycleCoverageInvariant,
             AsyncCandidateProducerContinuationLifecycleCoverageInvariantIn,
             AsyncCandidateProducerContinuationLifecycleCoveredIn,
             AsyncRetransmitLifecycleOrdinal,
             AsyncRetransmitLifecyclePhysicalCut,
             AsyncRetransmitLifecycleOwned,
             RetransmitDue, RetransmitTagPresent,
             AsyncOlderCandidateLifecycleBlocksRetransmit,
             AsyncEffectiveRetransmitLifecycleOrdinal,
             AsyncEffectiveRetransmitLifecyclePhysicalCut,
             TimeoutDue, AsyncOlderRuntimeLifecycleBlocksTimeout,
             AsyncOlderRetransmitLifecycleBlocksTimeout,
             AsyncOlderCandidateLifecycleBlocksTimeout,
             AsyncTimeoutClockDue, AsyncTimeoutClockDueIn,
             TimeoutTagPresentIn, ResponsiveReplayQuarantinedIn,
             AsyncNextCandidateLifecycleOrdinal
    <2>3bb. AsyncCandidateLifecycleSchedulerCoverageInvariant
      BY <1>1, Isa
         DEF AsyncInitAt, AsyncBaseInitAt, AsyncTransportInit,
             AsyncRuntimeInit, AsyncIoInit, AsyncDeferredInit,
             AsyncCandidateLifecycleSchedulerCoverageInvariant,
             AsyncCandidateLifecycleActiveRecords,
             AsyncCandidateLifecycleRecordCoversScheduledOrigin,
             AsyncScheduledCandidateOriginsForNode,
             AsyncCandidateLifecycleAdmissions,
             AsyncInitialCandidateLifecycleAdmissions,
             QueuedCandidates, DeferredCandidates,
             CausalCandidates, TrackedWorkCandidates,
             SequenceSet
    <2>3a. AsyncCertifiedResponseClaimIngressOwnershipInvariant
      BY <1>1, EmptyCertifiedResponseClaimHasIngressOwnership
         DEF AsyncInitAt, AsyncBaseInitAt, AsyncTransportInit
    <2>3d. AsyncLeaderWireIngressCarrierOwnershipInvariant
      BY <1>1,
         AsyncInitEstablishesLeaderWireIngressCarrierOwnership
    <2>3e. AsyncOrdinaryIngressCarrierOwnershipInvariant
      BY <1>1,
         AsyncInitEstablishesOrdinaryIngressCarrierOwnership
    <2>3f. AsyncProducerTypeInvariant
      BY <1>1, AsyncInitEstablishesProducerTypeInvariant
    <2>3p. /\ AsyncServeProducerTurnTypeInvariant
             /\ AsyncServeProducerTurnOwnershipInvariant
      BY <1>1, AsyncInitEstablishesServeProducerTurnInvariants
    <2>3t. AsyncTimeoutRecoveryEpisodeCurrentBoundaryInvariant
      BY <1>1, AsyncInitEstablishesTimeoutRecoveryCurrentBoundary
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
    <2> QED BY <2>1, <2>3, <2>3a, <2>3b, <2>3bb, <2>3c, <2>3d, <2>3e,
                <2>3f, <2>3p, <2>3t, <2>4, <2>5, <2>6, <2>7
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
           /\ AsyncProducerTypeInvariant
           /\ AsyncServeProducerTurnTypeInvariant
           /\ AsyncServeProducerTurnOwnershipInvariant
           /\ AsyncServiceActivationPairInvariant
           /\ AsyncControlServiceStateTypeInvariant
           /\ AsyncTimeoutRecoveryEpisodeCurrentBoundaryInvariant
           /\ AsyncCandidateLifecycleSchedulerCoverageInvariant
           /\ AsyncCertifiedResponseClaimIngressOwnershipInvariant
           /\ AsyncLeaderWireIngressCarrierOwnershipInvariant
           /\ AsyncOrdinaryIngressCarrierOwnershipInvariant
           /\ ReceivedTimeoutVotePoolInvariant
           /\ AsyncRecoveryTypeInvariant
           /\ AsyncRestartAuthorityInvariant
           /\ AsyncRecoveryExecutionInvariant
           /\ AsyncHistoricalLockRestartAuthorityTypeInvariant
           /\ HistoricalLockRestartAuthoritySourceRetentionInvariant
           /\ AsyncGstRecoveryPhaseInvariant
           /\ AsyncSerializedBusyKernelInvariant
           /\ AsyncServeProducerTurnTypeInvariant
           /\ AsyncServeProducerTurnOwnershipInvariant
           /\ AsyncTimeoutRecoveryEpisodeCurrentBoundaryInvariant
      BY <1>1 DEF AsyncStrongTypeInvariant
    <2>2. UNCHANGED vars
      BY <1>1, Isa DEF AsyncAllVars
    <2>3. StrongInductiveInvariant'
      BY <2>1, <2>2, CoreStrongInductiveActionPreservation
    <2>4. AsyncSchedulerTypeInvariant'
      BY <1>1, <2>1, AsyncAllVarsStutterPreservesSchedulerType
    <2>4aa. AsyncServiceActivationPairInvariant'
      BY <1>1, <2>1, Isa
         DEF AsyncAllVars, AsyncSchedulerVars,
             AsyncServiceActivationPairInvariant,
             AsyncActiveServiceNodes
    <2>4b. AsyncControlServiceStateTypeInvariant'
      BY <1>1, <2>1, Isa
         DEF AsyncAllVars, AsyncSchedulerVars,
             AsyncControlServiceStateTypeInvariant,
             AsyncControlServiceSlots,
             AsyncNextControlServiceOrdinal,
             AsyncCertifiedResponseClaimRecords,
             AsyncNextCertifiedResponseClaimOrdinal,
             AsyncCandidateServiceTombstones,
             AsyncNextCandidateServiceOrdinal,
             AsyncRetransmitLifecycleOrdinal,
             AsyncRetransmitLifecyclePhysicalCut,
             AsyncRetransmitLifecycleOwned,
             RetransmitDue, RetransmitTagPresent,
             AsyncOlderCandidateLifecycleBlocksRetransmit,
             AsyncEffectiveRetransmitLifecycleOrdinal,
             AsyncEffectiveRetransmitLifecyclePhysicalCut,
             TimeoutDue, AsyncOlderRuntimeLifecycleBlocksTimeout,
             AsyncOlderRetransmitLifecycleBlocksTimeout,
             AsyncOlderCandidateLifecycleBlocksTimeout,
             AsyncTimeoutClockDue, AsyncTimeoutClockDueIn,
             TimeoutTagPresentIn, ResponsiveReplayQuarantinedIn,
             AsyncNextCandidateLifecycleOrdinal
    <2>4bb. AsyncCandidateLifecycleSchedulerCoverageInvariant'
      BY <1>1, <2>1, Isa
         DEF AsyncAllVars, AsyncSchedulerVars,
             AsyncCandidateLifecycleSchedulerCoverageInvariant,
             AsyncCandidateLifecycleActiveRecords,
             AsyncCandidateLifecycleRecordCoversScheduledOrigin,
             AsyncScheduledCandidateOriginsForNode,
             AsyncCandidateLifecycleAdmissions
    <2>4a. AsyncCertifiedResponseClaimIngressOwnershipInvariant'
      BY <1>1, <2>1,
         CertifiedResponseClaimIngressOwnershipStutter
         DEF AsyncAllVars, AsyncSchedulerVars
    <2>4c. AsyncLeaderWireIngressCarrierOwnershipInvariant'
      BY <1>1, <2>1, Isa
         DEF AsyncAllVars, AsyncSchedulerVars,
             AsyncLeaderWireIngressCarrierOwnershipInvariant,
             AsyncLeaderWireIngressCarrierCoordinates,
             AsyncChunkExactLifecycleCoordinatesRetained,
             IngressLane
    <2>4d. AsyncOrdinaryIngressCarrierOwnershipInvariant'
      BY <1>1, <2>1, Isa
         DEF AsyncAllVars, AsyncSchedulerVars,
             AsyncOrdinaryIngressCarrierOwnershipInvariant,
             AsyncOrdinaryIngressCarrierCoordinates,
             IngressLane
    <2>4e. AsyncProducerTypeInvariant'
      BY <1>1, <2>1, AsyncProducerVarsFramePreservesTypeInvariant
         DEF AsyncAllVars
    <2>4f. AsyncServeProducerTurnTypeInvariant'
      BY <1>1, <2>1,
         AsyncServeProducerTurnFramePreservesTypeInvariant
         DEF AsyncAllVars
    <2>4g. AsyncServeProducerTurnOwnershipInvariant'
      BY <1>1, <2>1, Isa
         DEF AsyncAllVars, AsyncSchedulerVars,
             AsyncServeProducerTurnOwnershipInvariant,
             AsyncServeIngressLifecycleOwnerIdentities,
             AsyncServeIngressAdmissionIdentities,
             AsyncServeOffQueueReservations
    <2>4h. AsyncTimeoutRecoveryEpisodeCurrentBoundaryInvariant'
      BY <1>1, <2>1, Isa
         DEF AsyncAllVars, AsyncSchedulerVars, vars,
             AsyncTimeoutRecoveryEpisodeCurrentBoundaryInvariant,
             AsyncTimeoutRecoveryEpisodes,
             AsyncTimeoutRecoveryEpisodesIn, NodeHasDecision
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
             AsyncServeIngressLifecycleOwnerIdentities,
             AsyncServeIngressAdmissionIdentities,
             AsyncHistoricalLockRestartAuthorityTypeInvariant,
             HistoricalLockRestartAuthoritySourceRetentionInvariant
    <2>7. AsyncGstRecoveryPhaseInvariant'
      BY <1>1, <2>1, Isa
         DEF AsyncAllVars, AsyncRecoveryVars, vars,
             AsyncGstRecoveryPhaseInvariant
    <2>8. AsyncSerializedBusyKernelInvariant'
      BY <2>1, <2>2,
         CoreVarsStutterPreservesSerializedBusyKernelInvariant
    <2>8p. /\ AsyncServeProducerTurnTypeInvariant'
             /\ AsyncServeProducerTurnOwnershipInvariant'
      BY <1>1, <2>1, Isa
         DEF AsyncAllVars, AsyncSchedulerVars,
             AsyncServeProducerTurnTypeInvariant,
             AsyncServeProducerTurnOwnershipInvariant,
             AsyncServeIngressLifecycleOwnerIdentities,
             AsyncServeIngressAdmissionIdentities,
             AsyncServeOffQueueReservations,
             AsyncServeJobQueued, AsyncIoServeIdentities
    <2>8t. AsyncTimeoutRecoveryEpisodeCurrentBoundaryInvariant'
      BY <1>1, <2>1, Isa
         DEF AsyncAllVars, AsyncSchedulerVars, vars,
             AsyncTimeoutRecoveryEpisodeCurrentBoundaryInvariant,
             AsyncTimeoutRecoveryEpisodes,
             AsyncTimeoutRecoveryEpisodesIn,
             NodeHasDecision, AsyncNodeHasDecisionIn
    <2> QED BY <2>3, <2>4, <2>4aa, <2>4a, <2>4b, <2>4bb, <2>4c, <2>4d,
                <2>4e, <2>5, <2>6, <2>7, <2>8, <2>8p, <2>8t
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

THEOREM AsyncServiceActivationActionPreservesSchedulerType ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ (AsyncEnterIndexedServiceActivation(node)
          \/ AsyncActivateServiceNode(node))
    => AsyncSchedulerTypeInvariant'
BY FunctionalUpdatePreservesType, IsaT(600)
   DEF AsyncEnterIndexedServiceActivation,
       AsyncActivateServiceNode,
       AsyncServiceActivationFrameVars,
       AsyncSchedulerExceptServiceActivation,
       AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
       AsyncIoTypeInvariant, AsyncDeferredTypeInvariant,
       AsyncTransportTypeInvariant,
       AsyncTransportClockTypeInvariant,
       AsyncIngressTypeInvariant,
       AsyncHistoricalRecoveryTypeInvariant,
       AsyncConfiguration

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
    <2>2. CASE \E node \in ValidatorIds:
                  AsyncEnterIndexedServiceActivation(node)
      <3>1. PICK node \in ValidatorIds:
               AsyncEnterIndexedServiceActivation(node)
        BY <2>2
      <3> QED BY <1>1, <3>1,
           AsyncServiceActivationActionPreservesSchedulerType
    <2>3. CASE \E node \in ValidatorIds:
                  AsyncActivateServiceNode(node)
      <3>1. PICK node \in ValidatorIds:
               AsyncActivateServiceNode(node)
        BY <2>3
      <3> QED BY <1>1, <3>1,
           AsyncServiceActivationActionPreservesSchedulerType
    <2>4. CASE \E node \in ValidatorIds: PreGstCrash(node)
      BY <1>1, <2>4, PreGstCrashPreservesSchedulerType
         DEF AsyncTypeInvariant
    <2>5. CASE \E node \in ValidatorIds: PreGstResponsiveCrash(node)
      BY <1>1, <2>5, PreGstResponsiveCrashPreservesSchedulerType
         DEF AsyncTypeInvariant
    <2>6. CASE PreGstResponsiveRestart
      BY <1>1, <2>6, PreGstResponsiveRestartPreservesSchedulerType
         DEF AsyncTypeInvariant
    <2>7. CASE PreGstResponsiveReplay
      BY <1>1, <2>7, PreGstResponsiveReplayPreservesSchedulerType
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4, <2>5, <2>6,
                  <2>7
         DEF AsyncNext
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

(***************************************************************************
Replay entry removes every active exact-Serve ingress selector owner for the
recovering node.  Every persisted waiter is discharged locally: an unsealed
admission becomes its exact typed terminal, while a terminal Response waiter
is revalidated for exact replay or atomically converted to the Decision
outcome.  No requester-dependent dormant debt survives.  Volatile
policy-rejection occurrences are discarded.  The only transition which can
add an active admission is `AdmitHiddenPacket`; replay quarantine excludes the
recovering node from that action.  Drain and receiver-close transitions only
transform or remove existing active records.  The lemmas below make that
transition audit explicit before the execution invariant consumes it.
***************************************************************************)
THEOREM ServeIngressAdmissionStutterPreservesOwnerIdentities ==
  \A owner:
    UNCHANGED asyncServeIngressAdmissions
      => AsyncServeIngressLifecycleOwnerIdentities(owner)' =
           AsyncServeIngressLifecycleOwnerIdentities(owner)
BY Isa
   DEF AsyncServeIngressLifecycleOwnerIdentities,
       AsyncServeIngressAdmissionIdentities

THEOREM PopSelectedIngressDoesNotCreateServeIngressOwners ==
  \A owner, node, index, laneIndex:
    PopSelectedIngress(node, index, laneIndex)
      => AsyncServeIngressLifecycleOwnerIdentities(owner)'
           \subseteq AsyncServeIngressLifecycleOwnerIdentities(owner)
BY IsaT(180)
   DEF PopSelectedIngress,
       AsyncServeIngressAdmissionsAfterIngressDrain,
       AsyncServeIngressLifecycleOwnerIdentities,
       AsyncServeIngressAdmissionIdentities,
       AsyncServeIngressAdmission

THEOREM ServeReceiverCloseRollbackDoesNotCreateIngressOwners ==
  \A owner, node, identity:
    PreGstServeReceiverCloseRollback(node, identity)
      => AsyncServeIngressLifecycleOwnerIdentities(owner)'
           \subseteq AsyncServeIngressLifecycleOwnerIdentities(owner)
BY IsaT(180)
   DEF PreGstServeReceiverCloseRollback,
       PreGstPendingServeReceiverCloseRollback,
       PreGstMaterializedServeReceiverCloseRollback,
       AsyncServeIngressAdmissionsWithout,
       AsyncServeIngressLifecycleOwnerIdentities,
       AsyncServeIngressAdmissionIdentities,
       AsyncServeIngressAdmissionVars

THEOREM HiddenIngressAdmissionPreservesOtherNodeOwners ==
  \A recipient, source, owner:
    /\ recipient # owner
    /\ AdmitHiddenPacket(recipient, source)
    => AsyncServeIngressLifecycleOwnerIdentities(owner)' =
         AsyncServeIngressLifecycleOwnerIdentities(owner)
BY IsaT(240)
   DEF AdmitHiddenPacket, AcceptOrReserveExactServeIngress,
       ReserveExactServeCapacity, AdvanceExactServeCapacity,
       CoalesceExactServeIngressCapacity,
       AsyncServeIngressLifecycleOwnerIdentities,
       AsyncServeIngressAdmissionIdentities,
       AsyncServeIngressAdmission,
       AsyncIoVars, AsyncServeLifecycleVars,
       AsyncServeIngressAdmissionVars

THEOREM ReplayingNetworkStepPreservesEmptyRecoveryIngressOwners ==
  /\ asyncRecoveryPhase = "Replaying"
  /\ AsyncServeIngressLifecycleOwnerIdentities(asyncRecoveryNode) = {}
  /\ AsyncNetworkStep
  /\ UNCHANGED AsyncRecoveryControlVars
  => AsyncServeIngressLifecycleOwnerIdentities(asyncRecoveryNode)' = {}
BY HiddenIngressAdmissionPreservesOtherNodeOwners,
   ServeIngressAdmissionStutterPreservesOwnerIdentities,
   IsaT(180)
   DEF AsyncNetworkStep, AdmitIngressPacket,
       AdmitHiddenPacket, CoalesceHiddenPacket,
       DropPolicyRejectedHiddenPacket,
       ResponsiveReplayQuarantined,
       AsyncIoVars, AsyncServeIngressAdmissionVars

THEOREM FaultStepPreservesEmptyServeIngressOwners ==
  \A owner:
    /\ AsyncServeIngressLifecycleOwnerIdentities(owner) = {}
    /\ AsyncFaultStep
    => AsyncServeIngressLifecycleOwnerIdentities(owner)' = {}
BY ServeReceiverCloseRollbackDoesNotCreateIngressOwners,
   ServeIngressAdmissionStutterPreservesOwnerIdentities,
   IsaM("blast")
   DEF AsyncFaultStep, PreGstLosePacket, PreGstCrash,
       InjectByzantineNoise, InjectUntrustedTransportCompletion,
       InjectAuthenticatedJunk, InjectByzantineCertifiedRequest,
       AsyncByzantineProposal, AsyncByzantineVote,
       AsyncByzantineTimeout, AsyncSchedulerVars,
       AsyncIoVars, AsyncServeIngressAdmissionVars,
       AsyncDeferredVars, AsyncLocalAdmissionVars,
       LeaveCausalQueues

THEOREM NonRunnerStepPreservesEmptyReplayingIngressOwners ==
  /\ asyncRecoveryPhase = "Replaying"
  /\ AsyncServeIngressLifecycleOwnerIdentities(asyncRecoveryNode) = {}
  /\ AsyncNonRunnerStep
  /\ UNCHANGED AsyncRecoveryControlVars
  => AsyncServeIngressLifecycleOwnerIdentities(asyncRecoveryNode)' = {}
BY ReplayingNetworkStepPreservesEmptyRecoveryIngressOwners,
   FaultStepPreservesEmptyServeIngressOwners,
   ServeIngressAdmissionStutterPreservesOwnerIdentities,
   IsaM("blast")
   DEF AsyncNonRunnerStep, AsyncSetGST, AsyncTick,
       AsyncNonClockVars, OpenHistoricalRecovery,
       DirectCommitCertificateDiscoveryStep,
       DirectHistoricalCommitCertificateDiscoveryStep,
       CommitCertificateDiscoveryStepWork,
       ServiceIoWorker, ServiceHistoricalRecoveryIoWorker,
       ServiceIoWorkerWork,
       EnqueueIoLocalControl, EnqueueHistoricalRecoveryIoLocalControl,
       EnqueueIoLocalControlWork,
       AsyncSchedulerVars,
       AsyncSchedulerExceptHistoricalRecoveryTargets,
       AsyncIoVars, AsyncServeIngressAdmissionVars,
       AsyncRecoveryVars

THEOREM ReplayRunNodeContinuationPreservesEmptyServeIngressOwners ==
  \A owner, node:
    /\ AsyncServeIngressLifecycleOwnerIdentities(owner) = {}
    /\ ReplayRunNodeCandidateProducerContinuation(node)
    => AsyncServeIngressLifecycleOwnerIdentities(owner)' = {}
BY ServeIngressAdmissionStutterPreservesOwnerIdentities, IsaT(180)
   DEF ReplayRunNodeCandidateProducerContinuation,
       AsyncCandidateProducerContinuationExactLocalReplayStep,
       AsyncCandidateProducerContinuationReplayTargetOnlyTurn,
       AsyncCandidateProducerContinuationExactRuntimeReplayStep,
       AsyncIoTimeoutLifecycleRetirementTransition,
       AsyncSchedulerExceptCausalControlCommandRunnerAndNodeService,
       AsyncSchedulerExceptCausalControlRunnerAndNodeService,
       AsyncIoVars, AsyncServeIngressAdmissionVars,
       AsyncServeLifecycleVars, AsyncDeferredVars,
       AsyncLocalAdmissionVars, vars

THEOREM RunNodeWorkPreservesEmptyServeIngressOwners ==
  \A owner, node:
    /\ AsyncServeIngressLifecycleOwnerIdentities(owner) = {}
    /\ RunNodeWork(node)
    => AsyncServeIngressLifecycleOwnerIdentities(owner)' = {}
BY PopSelectedIngressDoesNotCreateServeIngressOwners,
   ReplayRunNodeContinuationPreservesEmptyServeIngressOwners,
   ServeIngressAdmissionStutterPreservesOwnerIdentities,
   IsaT(300)
   DEF RunNodeWork, LocalAdmissionStep,
       AdmitProducerCompletion, AdmitCausalHead,
       IngressDrainStep, DrainFairIngressSelected,
       SerializedRunnerRuntimeStep, SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       SerializedLocalPrecedesServeIngressStep,
       SelectedLocalAdmissionAdvance,
       AsyncServeIngressTargetOnlyTurn,
       ResolveRunNodeCandidateProducerContinuation,
       AsyncSchedulerExceptCausalControlAndNodeService,
       AsyncIoVars, AsyncServeIngressAdmissionVars,
       AsyncServeLifecycleVars, AsyncDeferredVars,
       AsyncLocalAdmissionVars, vars

THEOREM HistoricalServerPreservesEmptyServeIngressOwners ==
  \A owner, node:
    /\ AsyncServeIngressLifecycleOwnerIdentities(owner) = {}
    /\ RunHistoricalServer(node)
    => AsyncServeIngressLifecycleOwnerIdentities(owner)' = {}
BY PopSelectedIngressDoesNotCreateServeIngressOwners,
   ServeIngressAdmissionStutterPreservesOwnerIdentities,
   IsaT(180)
   DEF RunHistoricalServer, DrainHistoricalIngressSelected,
       HistoricalIdleStep, AsyncIoVars,
       AsyncServeIngressAdmissionVars

THEOREM RunnerStepPreservesEmptyServeIngressOwners ==
  \A owner:
    /\ AsyncServeIngressLifecycleOwnerIdentities(owner) = {}
    /\ AsyncRunnerStep
    => AsyncServeIngressLifecycleOwnerIdentities(owner)' = {}
BY RunNodeWorkPreservesEmptyServeIngressOwners,
   HistoricalServerPreservesEmptyServeIngressOwners,
   Isa
   DEF AsyncRunnerStep, RunNode, RunHistoricalRecoveryNode

THEOREM AsyncNetworkStepPreservesEmptyServeIngressOwnersWhileProducerTurnReady ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ asyncServeProducerTurnReady[node]
    /\ AsyncServeIngressLifecycleOwnerIdentities(node) = {}
    /\ AsyncNetworkStep
    => AsyncServeIngressLifecycleOwnerIdentities(node)' = {}
BY AsyncServeProducerTurnBlocksFreshServeAdmission,
   PopSelectedIngressDoesNotCreateServeIngressOwners,
   HiddenIngressAdmissionPreservesOtherNodeOwners,
   ServeIngressAdmissionStutterPreservesOwnerIdentities,
   IsaT(3600)
   DEF AsyncNetworkStep, AdmitIngressPacket, AdmitHiddenPacket,
       AcceptOrReserveExactServeIngressVia,
       ReserveExactServeCapacityVia, AdvanceExactServeCapacityVia,
       CoalesceHiddenPacket, DropPolicyRejectedHiddenPacket,
       DropExactActiveLeaderWireRetry,
       DrainInterruptedTipRecoveryIngressSelected,
       RetireLeaderWireLifecycleSlot,
       AsyncServeLifecycleAdmissionRequired,
       ExactServeTransportAdmissionCanAdvanceVia,
       AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncIoTypeInvariant, AsyncServeLifecycleTypeInvariant,
       AsyncIoVars, AsyncIoExceptServeAttemptsVars,
       AsyncServeLifecycleVars, AsyncServeIngressAdmissionVars

THEOREM AsyncNextPreservesEmptyServeIngressOwnersWhileProducerTurnReady ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ asyncServeProducerTurnReady[node]
    /\ AsyncServeIngressLifecycleOwnerIdentities(node) = {}
    /\ AsyncNext
    => AsyncServeIngressLifecycleOwnerIdentities(node)' = {}
BY AsyncNetworkStepPreservesEmptyServeIngressOwnersWhileProducerTurnReady,
   RunnerStepPreservesEmptyServeIngressOwners,
   FaultStepPreservesEmptyServeIngressOwners,
   PopSelectedIngressDoesNotCreateServeIngressOwners,
   ServeReceiverCloseRollbackDoesNotCreateIngressOwners,
   HiddenIngressAdmissionPreservesOtherNodeOwners,
   ServeIngressAdmissionStutterPreservesOwnerIdentities,
   IsaT(7200)
   DEF AsyncNext, AsyncNonCrashStep, AsyncNonRunnerStep,
       AsyncNetworkStep, AsyncFaultStep,
       AdmitIngressPacket, AdmitHiddenPacket,
       AcceptOrReserveExactServeIngressVia,
       ReserveExactServeCapacityVia, AdvanceExactServeCapacityVia,
       CoalesceHiddenPacket, DropPolicyRejectedHiddenPacket,
       DropExactActiveLeaderWireRetry,
       DrainInterruptedTipRecoveryIngressSelected,
       RetireLeaderWireLifecycleSlot,
       AsyncSetGST, AsyncTick, AsyncNonClockVars,
       OpenHistoricalRecovery,
       DirectCommitCertificateDiscoveryStep,
       DirectHistoricalCommitCertificateDiscoveryStep,
       CommitCertificateDiscoveryStepWork,
       ServiceIoWorker, ServiceHistoricalRecoveryIoWorker,
       ServiceIoWorkerWork,
       EnqueueIoLocalControl, EnqueueHistoricalRecoveryIoLocalControl,
       EnqueueIoLocalControlWork,
       ResolveCandidateProducerContinuation,
       DriveResponsiveReplayHead, FinishResponsiveReplay,
       RearmResponsiveRecovery,
       AsyncEnterIndexedServiceActivation,
       AsyncActivateServiceNode, AsyncServiceActivationFrameVars,
       PreGstCrash, PreGstResponsiveCrash,
       PreGstResponsiveRestart, PreGstResponsiveReplay,
       ResetNodeSchedulerForRestart,
       AsyncSchedulerVars,
       AsyncSchedulerExceptCausalAndControlService,
       AsyncSchedulerExceptHistoricalRecoveryTargets,
       AsyncIoVars, AsyncDeferredVars, AsyncLocalAdmissionVars,
       AsyncServeLifecycleVars, AsyncServeIngressAdmissionVars

THEOREM ReplayingOrdinaryStepPreservesEmptyServeIngressOwners ==
  /\ AsyncRecoveryExecutionInvariant
  /\ asyncRecoveryPhase = "Replaying"
  /\ (AsyncRunnerStep \/ AsyncNonRunnerStep)
  /\ UNCHANGED AsyncRecoveryControlVars
  => AsyncServeIngressLifecycleOwnerIdentities(asyncRecoveryNode)' = {}
BY RunnerStepPreservesEmptyServeIngressOwners,
   NonRunnerStepPreservesEmptyReplayingIngressOwners,
   Isa
   DEF AsyncRecoveryExecutionInvariant, AsyncRecoveryControlVars

THEOREM AsyncServiceActivationActionPreservesRecoveryInvariants ==
  \A node \in ValidatorIds:
    /\ AsyncRecoveryTypeInvariant
    /\ AsyncRestartAuthorityInvariant
    /\ (AsyncEnterIndexedServiceActivation(node)
          \/ AsyncActivateServiceNode(node))
    => /\ AsyncRecoveryTypeInvariant'
       /\ AsyncRestartAuthorityInvariant'
BY IsaT(300)
   DEF AsyncEnterIndexedServiceActivation,
       AsyncActivateServiceNode,
       AsyncServiceActivationFrameVars,
       AsyncSchedulerExceptServiceActivation,
       AsyncRecoveryTypeInvariant,
       AsyncRestartAuthorityInvariant,
       AsyncRecoveryVars

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
    <2>6. CASE \E node \in ValidatorIds:
                  AsyncEnterIndexedServiceActivation(node)
      <3>1. PICK node \in ValidatorIds:
               AsyncEnterIndexedServiceActivation(node)
        BY <2>6
      <3> QED BY <1>1, <3>1,
           AsyncServiceActivationActionPreservesRecoveryInvariants
    <2>7. CASE \E node \in ValidatorIds:
                  AsyncActivateServiceNode(node)
      <3>1. PICK node \in ValidatorIds:
               AsyncActivateServiceNode(node)
        BY <2>7
      <3> QED BY <1>1, <3>1,
           AsyncServiceActivationActionPreservesRecoveryInvariants
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4, <2>5, <2>6,
                  <2>7
         DEF AsyncNext
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

THEOREM SerializedLocalPredecessorLeavesOutstandingTags ==
  \A node:
    SerializedLocalPrecedesServeIngressStep(node)
      => UNCHANGED asyncOutstandingTags
BY Isa
   DEF SerializedLocalPrecedesServeIngressStep,
       SelectedLocalAdmissionAdvance,
       AdmitProducerCompletion, AdmitCausalHead,
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

THEOREM ReplayRunNodeContinuationLeavesOutstandingTags ==
  \A node:
    ReplayRunNodeCandidateProducerContinuation(node)
      => UNCHANGED asyncOutstandingTags
PROOF
  <1>1. ASSUME NEW node,
                ReplayRunNodeCandidateProducerContinuation(node)
         PROVE UNCHANGED asyncOutstandingTags
    <2>1. CASE
              AsyncCandidateProducerContinuationExactLocalReplayStep(node)
      BY <2>1, Isa
         DEF AsyncCandidateProducerContinuationExactLocalReplayStep,
             AsyncSchedulerExceptCausalControlCommandRunnerAndNodeService
    <2>2. CASE
              AsyncCandidateProducerContinuationReplayTargetOnlyTurn(node)
      BY <2>2, Isa
         DEF AsyncCandidateProducerContinuationReplayTargetOnlyTurn,
             AsyncSchedulerExceptCausalControlRunnerAndNodeService
    <2>3. CASE
              AsyncCandidateProducerContinuationExactRuntimeReplayStep(node)
      BY <2>3, DeferredDrainStepLeavesOutstandingTags,
         FifoRuntimeStepLeavesOutstandingTags, Isa
         DEF AsyncCandidateProducerContinuationExactRuntimeReplayStep
    <2> QED BY <1>1, <2>1, <2>2, <2>3
         DEF ReplayRunNodeCandidateProducerContinuation
  <1> QED BY <1>1

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
    <2>1r. CASE
              ResolveRunNodeCandidateProducerContinuation(node)
      BY <2>1r, Isa
         DEF ResolveRunNodeCandidateProducerContinuation,
             AsyncSchedulerExceptCausalControlAndNodeService
    <2>1p. CASE
              ReplayRunNodeCandidateProducerContinuation(node)
      BY <2>1p, ReplayRunNodeContinuationLeavesOutstandingTags
    <2>2. CASE LocalAdmissionStep(node)
      BY <2>2, LocalAdmissionStepLeavesOutstandingTags
    <2>3. CASE IngressDrainStep(node)
      BY <2>3, IngressDrainStepLeavesOutstandingTags
    <2>4. CASE SerializedRuntimeStep(node)
                  \/ SerializedRuntimePrecedesServeIngressStep(node)
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
           DEF SerializedRuntimeStep,
               SerializedRuntimePrecedesServeIngressStep,
               RuntimeStep
    <2>5. CASE AsyncServeIngressTargetOnlyTurn(node)
      BY <2>5, Isa DEF AsyncServeIngressTargetOnlyTurn, vars
    <2>6. CASE SerializedLocalPrecedesServeIngressStep(node)
      BY <2>6, SerializedLocalPredecessorLeavesOutstandingTags
    <2> QED BY <1>1, <2>1r, <2>1p, <2>2, <2>3, <2>4, <2>5,
                 <2>6
         DEF RunNodeWork
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

THEOREM ReplayingSerializedLocalPredecessorDoesNotCreateRecoveryCandidate ==
  \A node:
    /\ AsyncTypeInvariant
    /\ AsyncRecoveryTypeInvariant
    /\ asyncRecoveryPhase = "Replaying"
    /\ SerializedLocalPrecedesServeIngressStep(node)
    => ResponsiveReplayScheduledCandidates(asyncRecoveryNode)'
         \subseteq
           ResponsiveReplayScheduledCandidates(asyncRecoveryNode)
BY HeadTailProperties, SequenceSetAfterAppend, SMTT(30), Isa
   DEF SerializedLocalPrecedesServeIngressStep,
       SelectedLocalAdmissionAdvance,
       AdmitProducerCompletion, AdmitCausalHead,
       UpdateLocalAdmissionMetadata,
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
    /\ UNCHANGED AsyncRecoveryControlVars
    /\ (SerializedRuntimeStep(node)
          \/ SerializedRuntimePrecedesServeIngressStep(node)
          \/ AsyncCandidateProducerContinuationExactRuntimeReplayStep(node))
    => SequenceSet(asyncRecoveryReplayQueue)' \cap
         ResponsiveReplayScheduledCandidates(asyncRecoveryNode)' = {}
BY HeadTailProperties, SequenceSetAfterAppend, SMTT(45), Isa
   DEF SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       AsyncCandidateProducerContinuationExactRuntimeReplayStep,
       RuntimeStep,
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
       SequenceSet, AsyncRecoveryControlVars, vars

THEOREM ReplayingContinuationExactLocalReplayDoesNotCreateRecoveryCandidate ==
  \A node:
    /\ AsyncTypeInvariant
    /\ AsyncRecoveryTypeInvariant
    /\ asyncRecoveryPhase = "Replaying"
    /\ ReplayRunNodeCandidateProducerContinuation(node)
    /\ AsyncCandidateProducerContinuationExactLocalReplayStep(node)
    => ResponsiveReplayScheduledCandidates(asyncRecoveryNode)'
         \subseteq
           ResponsiveReplayScheduledCandidates(asyncRecoveryNode)
BY SequenceSetAfterAppend, SMTT(45), Isa
   DEF ReplayRunNodeCandidateProducerContinuation,
       AsyncCandidateProducerContinuationExactLocalReplayStep,
       AsyncCandidateProducerContinuationExactReplayIdentity,
       AsyncCandidateProducerContinuationSelectedLocalCandidate,
       AsyncCandidateProducerContinuationSelectedReplayRecord,
       AsyncCandidateProducerContinuationSelectedResolutionRecord,
       AsyncCandidateProducerContinuationResolutionRequired,
       AsyncCandidateProducerContinuationResolutionRecordsForNode,
       AsyncCandidateProducerContinuations,
       AsyncCandidateProducerContinuationRecordSet,
       AsyncCandidateProducerContinuationRecord,
       ResponsiveReplayQuarantined,
       EnqueueCandidate,
       ResponsiveReplayScheduledCandidates,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, CandidateScheduled,
       AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
       AsyncRecoveryTypeInvariant, AsyncCommandQueueOwnership,
       SequenceSet, vars

THEOREM ReplayingReplayRunNodeContinuationPreservesRecoveryCandidateFreshness ==
  \A node:
    /\ StrongInductiveInvariant
    /\ AsyncTypeInvariant
    /\ AsyncRecoveryTypeInvariant
    /\ AsyncRecoveryExecutionInvariant
    /\ asyncRecoveryPhase = "Replaying"
    /\ UNCHANGED AsyncRecoveryControlVars
    /\ ReplayRunNodeCandidateProducerContinuation(node)
    => SequenceSet(asyncRecoveryReplayQueue)' \cap
         ResponsiveReplayScheduledCandidates(asyncRecoveryNode)' = {}
PROOF
  <1>1. ASSUME NEW node,
                StrongInductiveInvariant,
                AsyncTypeInvariant,
                AsyncRecoveryTypeInvariant,
                AsyncRecoveryExecutionInvariant,
                asyncRecoveryPhase = "Replaying",
                UNCHANGED AsyncRecoveryControlVars,
                ReplayRunNodeCandidateProducerContinuation(node)
         PROVE SequenceSet(asyncRecoveryReplayQueue)' \cap
                 ResponsiveReplayScheduledCandidates(
                   asyncRecoveryNode)' = {}
    <2>1. CASE
              AsyncCandidateProducerContinuationExactLocalReplayStep(node)
      BY <1>1, <2>1,
         ReplayingContinuationExactLocalReplayDoesNotCreateRecoveryCandidate,
         Isa
         DEF AsyncRecoveryExecutionInvariant,
             AsyncRecoveryControlVars, AsyncRecoveryVars
    <2>2. CASE
              AsyncCandidateProducerContinuationReplayTargetOnlyTurn(node)
      BY <1>1, <2>2, Isa
         DEF AsyncCandidateProducerContinuationReplayTargetOnlyTurn,
             AsyncRecoveryExecutionInvariant,
             ResponsiveReplayScheduledCandidates,
             QueuedCandidates, DeferredCandidates, CausalCandidates,
             TrackedWorkCandidates,
             AsyncRecoveryControlVars, AsyncRecoveryVars, SequenceSet, vars
    <2>3. CASE
              AsyncCandidateProducerContinuationExactRuntimeReplayStep(node)
      BY <1>1, <2>3,
         ReplayingSerializedRuntimePreservesRecoveryCandidateFreshness
    <2> QED BY <1>1, <2>1, <2>2, <2>3
         DEF ReplayRunNodeCandidateProducerContinuation
  <1> QED BY <1>1

THEOREM ReplayingRunNodeWorkPreservesRecoveryCandidateFreshness ==
  \A node:
    /\ StrongInductiveInvariant
    /\ AsyncTypeInvariant
    /\ AsyncRecoveryTypeInvariant
    /\ AsyncRecoveryExecutionInvariant
    /\ asyncRecoveryPhase = "Replaying"
    /\ UNCHANGED AsyncRecoveryControlVars
    /\ RunNodeWork(node)
    => SequenceSet(asyncRecoveryReplayQueue)' \cap
         ResponsiveReplayScheduledCandidates(asyncRecoveryNode)' = {}
BY ReplayingLocalAdmissionDoesNotCreateRecoveryCandidate,
   ReplayingSerializedLocalPredecessorDoesNotCreateRecoveryCandidate,
   ReplayingIngressDrainDoesNotCreateRecoveryCandidate,
   ReplayingSerializedRuntimePreservesRecoveryCandidateFreshness,
   ReplayingReplayRunNodeContinuationPreservesRecoveryCandidateFreshness,
   Isa
   DEF RunNodeWork, SerializedRuntimePrecedesServeIngressStep,
       SerializedLocalPrecedesServeIngressStep,
       SelectedLocalAdmissionAdvance,
       AsyncServeIngressTargetOnlyTurn,
       ResolveRunNodeCandidateProducerContinuation,
       AsyncSchedulerExceptCausalControlAndNodeService,
       AsyncRecoveryExecutionInvariant,
       AsyncRecoveryControlVars, AsyncRecoveryVars, vars

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
        BY <1>1, <2>1, <3>2,
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
        BY <1>1, <2>1, <3>2,
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
    /\ (SerializedRuntimeStep(node)
          \/ SerializedRuntimePrecedesServeIngressStep(node))
    => asyncOutstandingTags'[node] = {}
PROOF
  <1>1. ASSUME NEW node,
                AsyncRecoveryExecutionInvariant,
                asyncRecoveryPhase = "Replaying",
                node = asyncRecoveryNode,
                SerializedRuntimeStep(node)
                  \/ SerializedRuntimePrecedesServeIngressStep(node)
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
         DEF SerializedRuntimeStep,
             SerializedRuntimePrecedesServeIngressStep, RuntimeStep
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
    <2>1a. \/ ResolveRunNodeCandidateProducerContinuation(node)
            \/ ReplayRunNodeCandidateProducerContinuation(node)
            \/ LocalAdmissionStep(node)
            \/ IngressDrainStep(node)
            \/ SerializedRunnerRuntimeStep(node)
            \/ SerializedLocalPrecedesServeIngressStep(node)
            \/ AsyncServeIngressTargetOnlyTurn(node)
      BY <1>1, RunNodeWorkConcreteActionCaseSplit
    <2>1r. CASE
              ResolveRunNodeCandidateProducerContinuation(node)
      BY <2>1, <2>1r, Isa
         DEF ResolveRunNodeCandidateProducerContinuation,
             AsyncSchedulerExceptCausalControlAndNodeService
    <2>1p. CASE
              ReplayRunNodeCandidateProducerContinuation(node)
      BY <2>1, <2>1p,
         ReplayRunNodeContinuationLeavesOutstandingTags
    <2>2. CASE LocalAdmissionStep(node)
      BY <2>1, <2>2, LocalAdmissionStepLeavesOutstandingTags
    <2>3. CASE IngressDrainStep(node)
      BY <2>1, <2>3, IngressDrainStepLeavesOutstandingTags
    <2>4. CASE SerializedRunnerRuntimeStep(node)
      BY <1>1, <2>4,
         ReplayingRecoveryNodeSerializedRuntimePreservesEmptyTags
         DEF SerializedRunnerRuntimeStep
    <2>5. CASE AsyncServeIngressTargetOnlyTurn(node)
      BY <2>1, <2>5, Isa
         DEF AsyncServeIngressTargetOnlyTurn, vars
    <2>6. CASE SerializedLocalPrecedesServeIngressStep(node)
      BY <2>1, <2>6,
         SerializedLocalPredecessorLeavesOutstandingTags
    <2> QED BY <2>1a, <2>1r, <2>1p, <2>2, <2>3, <2>4, <2>5,
                 <2>6
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

THEOREM ResetNodeSchedulerForRestartDischargesServeIngressDebt ==
  \A node, replay:
    ResetNodeSchedulerForRestart(node, replay)
      => /\ AsyncServeIngressLifecycleOwnerIdentities(node)' = {}
         /\ AsyncServeOffQueueReservations(node)' = {}
         /\ \A admission \in asyncServeAdmissions:
              admission.node = node
                => \E terminal \in asyncServeTombstones':
                     /\ terminal.node = admission.node
                     /\ terminal.identity = admission.identity
                     /\ terminal.family = admission.family
                     /\ terminal.view = admission.view
                     /\ terminal.ordinal = admission.ordinal
BY SameHeightRestartDischargesEveryLocalServeLifecycle,
   SameHeightRestartPreservesServeHighWatermarks,
   Isa
   DEF AsyncServeIngressLifecycleOwnerIdentities,
       AsyncServeIngressAdmissionIdentities

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
           /\ AsyncServeIngressLifecycleOwnerIdentities(Node) = {}
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
    <2>9. AsyncServeIngressLifecycleOwnerIdentities(Node)' = {}
      BY <1>1, <2>1,
         ServeIngressAdmissionStutterPreservesOwnerIdentities, Isa
         DEF DriveResponsiveReplayHead, AsyncIoVars,
             AsyncServeIngressAdmissionVars
    <2> QED BY <2>3, <2>4, <2>7, <2>8, <2>9
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
    <2>6b. AsyncServeIngressLifecycleOwnerIdentities(
              asyncRecoveryNode)' = {}
      BY <2>3,
         ResetNodeSchedulerForRestartDischargesServeIngressDebt
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
    <2> QED BY <2>5, <2>6, <2>6a, <2>6b, <2>7
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
          <5>1a. AsyncServeIngressLifecycleOwnerIdentities(
                    asyncRecoveryNode)' = {}
            BY <1>1, <4>1, <5>1,
               ReplayingOrdinaryStepPreservesEmptyServeIngressOwners
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
            <6> QED BY <2>2, <5>1, <5>1a, <6>1, <6>2, <6>3
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
            <6> QED BY <2>2, <5>1, <5>1a, <6>1, <6>2, <6>3,
                        <6>6
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
      <3>2. CASE \E node \in ValidatorIds:
                    AsyncEnterIndexedServiceActivation(node)
        BY <1>1, <2>2, <3>2, Isa
           DEF AsyncEnterIndexedServiceActivation,
               AsyncServiceActivationFrameVars,
               AsyncSchedulerExceptServiceActivation,
               AsyncRecoveryVars, AsyncRecoveryExecutionInvariant,
               AsyncServeIngressLifecycleOwnerIdentities,
               AsyncServeIngressAdmissionIdentities
      <3>3. CASE \E node \in ValidatorIds:
                    AsyncActivateServiceNode(node)
        BY <1>1, <2>2, <3>3, Isa
           DEF AsyncActivateServiceNode,
               AsyncServiceActivationFrameVars,
               AsyncSchedulerExceptServiceActivation,
               AsyncRecoveryVars, AsyncRecoveryExecutionInvariant,
               AsyncServeIngressLifecycleOwnerIdentities,
               AsyncServeIngressAdmissionIdentities
      <3>4. CASE \E node \in ValidatorIds: PreGstCrash(node)
        <4>1. PICK node \in ValidatorIds: PreGstCrash(node)
          BY <3>4
        <4> QED BY <1>1, <2>2, <4>1, Isa
             DEF PreGstCrash, AsyncSchedulerVars, AsyncRecoveryVars,
                 AsyncRecoveryExecutionInvariant,
                 AsyncServeIngressLifecycleOwnerIdentities,
                 AsyncServeIngressAdmissionIdentities
      <3>5. CASE \E node \in ValidatorIds:
                    PreGstResponsiveCrash(node)
        BY <2>2, <3>5, Isa DEF PreGstResponsiveCrash
      <3>6. CASE PreGstResponsiveRestart
        BY <2>2, <3>6, Isa DEF PreGstResponsiveRestart
      <3>7. CASE PreGstResponsiveReplay
        BY <1>1, <3>7,
           PreGstResponsiveReplayEstablishesRecoveryExecutionInvariant
      <3> QED BY <1>1, <2>2, <3>1, <3>2, <3>3, <3>4, <3>5,
                  <3>6, <3>7
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

THEOREM AsyncServiceActivationActionPreservesClaimIngressOwnership ==
  \A node \in ValidatorIds:
    /\ AsyncCertifiedResponseClaimIngressOwnershipInvariant
    /\ (AsyncEnterIndexedServiceActivation(node)
          \/ AsyncActivateServiceNode(node))
    => AsyncCertifiedResponseClaimIngressOwnershipInvariant'
BY CertifiedResponseClaimIngressOwnershipStutter, Isa
   DEF AsyncEnterIndexedServiceActivation,
       AsyncActivateServiceNode,
       AsyncServiceActivationFrameVars,
       AsyncSchedulerExceptServiceActivation

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
    <2>6. CASE \E node \in ValidatorIds:
                  AsyncEnterIndexedServiceActivation(node)
      <3>1. PICK node \in ValidatorIds:
               AsyncEnterIndexedServiceActivation(node)
        BY <2>6
      <3> QED BY <1>1, <3>1,
           AsyncServiceActivationActionPreservesClaimIngressOwnership
    <2>7. CASE \E node \in ValidatorIds:
                  AsyncActivateServiceNode(node)
      <3>1. PICK node \in ValidatorIds:
               AsyncActivateServiceNode(node)
        BY <2>7
      <3> QED BY <1>1, <3>1,
           AsyncServiceActivationActionPreservesClaimIngressOwnership
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4, <2>5, <2>6,
                  <2>7
         DEF AsyncNext
  <1> QED BY <1>1

THEOREM AsyncNextPreservesControlServiceStateTypeFromPrimedSchedulerType ==
  /\ AsyncTypeInvariant
  /\ AsyncControlServiceStateTypeInvariant
  /\ AsyncSchedulerTypeInvariant'
  /\ AsyncNext
  => AsyncControlServiceStateTypeInvariant'
BY AsyncCandidateServiceRecordProducersAreTrackedBoundaryKinds,
   AsyncCandidateProducerContinuationSourcePhysicalOrdinalIsBeforeCut,
   AsyncControlServiceTransitionPreservesCandidateProducerContinuationLifecycleCoverage,
   AsyncCandidateLifecycleDistinctNewRootsReceiveDistinctOwnership,
   AsyncCandidateLifecycleHighWatermarkAdvancesByFullFreshSet,
   AsyncServeIngressSharedHighWatermarkAdvancesByFreshTickets,
   AsyncServeIngressReservationPrecedesSameStepCandidateAllocation,
   AsyncRetransmitFreshEpisodeConsumesSharedLifecycleOrdinal,
   AsyncRetransmitFreshEpisodeAdvancesSharedHighWatermark,
   AsyncRetransmitCompletedEpisodeClearsActiveOwner,
   AsyncRetransmitCompletedOwnedEpisodeDefersFreshAcquisition,
   AsyncRetransmitFreshEpisodeCannotReuseDrainedPosition,
   AsyncRetransmitFreshLiveEpisodeRetainsSharedLifecycleOrdinal,
   AsyncFreshServeReservationPrecedesSameStepRetransmitAllocation,
   AsyncSharedSchedulerHighWatermarkIsMonotone,
   FunctionalUpdatePreservesType,
   FS_Subset, FS_Image, FS_Union, FS_Interval,
   FS_CardinalityType, IsaT(1800)
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
       AsyncCandidateProducerContinuationStateAfterDeparture,
       AsyncCandidateProducerContinuationRecord,
       AsyncCandidateProducerContinuationRecordSet,
       AsyncCandidateProducerContinuationSourcePhysicalOrdinalIn,
       AsyncCandidateProducerContinuationPhysicalCutIn,
       AsyncCandidateProducerContinuationLifecycleRecordIn,
       AsyncCandidateProducerContinuationAddressForIn,
       AsyncCandidateProducerContinuationOrdinalForIn,
       AsyncCandidateProducerContinuationInitialStatusAfter,
       AsyncCandidateLifecycleStateAfterServiceSlotTransfer,
       AsyncOrdinaryIngressCarrierStateAfterTransition,
       AsyncOrdinaryIngressCarrierEvidenceAfterPhysicalTransition,
       AsyncOrdinaryIngressCarrierAfterPhysicalTransition,
       AsyncCandidateLifecycleAdmissionsAfterOrdinaryIngressRebase,
       AsyncCandidateLifecycleAdmissionAfterOrdinaryIngressRebase,
       AsyncCandidateLifecycleStateAfterCarrierUpdate,
       AsyncCandidateLifecycleCarrierUpdatedAdmissions,
       AsyncCandidateLifecycleStateAfterCompaction,
       AsyncCandidateLifecycleRetirementCoveredIn,
       AsyncCandidateLifecyclePermanentlyObsoleteAfter,
       AsyncCandidateLifecycleDormantReservationOwnedAfter,
       AsyncCandidateLifecycleServiceRecordCoversIn,
       AsyncCandidateLifecycleStateAfterServeIngressAdmission,
       AsyncCandidateLifecycleStateAfterAdmission,
       AsyncCandidateLifecycleStateAfterTimeoutOwnership,
       AsyncCandidateLifecycleStateAfterOrdinaryIngressAdmission,
       AsyncCandidateLifecycleStateAfterLeaderWireAdmission,
       AsyncControlServiceResetNodesThisStep,
       AsyncControlServiceAdmissionsThisStep,
       AsyncControlServicesThisStep,
       AsyncCertifiedResponseClaimAdmissionsThisStep,
       AsyncCandidateServicesThisStep,
       AsyncCandidateSemanticallyAppliedThisStep,
       AsyncCandidateSuccessfullyServicedThisStep,
       AsyncCandidatePhysicallyDiscardedThisStep,
       AsyncCandidateTerminalRetirementsThisStep,
       AsyncCandidateTerminalDiscardsThisStep,
       AsyncCandidateTerminallyDiscardedThisStep,
       AsyncCandidateIgnoredWithoutApplicationThisStep,
       AsyncCandidateIgnoredWithoutApplicationThisStepSet,
       AsyncCandidateLifecycleDeparturesThisStep,
       AsyncCandidateServiceReservationAvailableIn,
       AsyncCandidateServiceSlotTransferNeededIn,
       AsyncCandidateLifecycleFirstFreeServicedSlotForIn,
       AsyncCandidateTerminalServiceReservationAvailableIn,
       AsyncCandidateTerminalServiceReservationNeededIn,
       AsyncCandidateServiceIdentityRecordedIn,
       AsyncCandidateTerminalRetirementEligibleAfterStep,
       AsyncCandidateLifecycleReservationsAvailableIn,
       AsyncCandidateLifecycleOrdinaryReservationsAvailableIn,
       AsyncCandidateLifecycleClockReservationAvailableIn,
       AsyncCandidateLifecycleNewAdmissions,
       AsyncCandidateLifecycleAdmissionOrdinalFor,
       AsyncCandidateLifecycleSourcePhysicalOrdinalFor,
       AsyncCandidateLifecyclePhysicalCutFor,
       AsyncCandidateLifecycleAdmissionSlotFor,
       AsyncCandidateLifecycleFreeSlotPredecessorsFor,
       AsyncCandidateLifecycleFreeOrdinarySlotsForNodeIn,
       AsyncCandidateLifecycleFreeServicedSlotsForNodeIn,
       AsyncCandidateLifecycleUsedOrdinarySlotsForNodeIn,
       AsyncCandidateLifecycleUsedActiveSlotsForNodeIn,
       AsyncCandidateLifecycleUsedServicedSlotsForNodeIn,
       AsyncCandidateLifecycleOrdinaryOriginsForNodeIn,
       AsyncOrdinaryNewCandidateLifecyclePredecessorsFor,
       AsyncOrdinaryNewCandidateLifecycleOriginsForNodeIn,
       AsyncNewCandidateLifecycleOriginsForNodeIn,
       AsyncCandidateLifecycleOriginsRecordedForNodeIn,
       AsyncCandidateLifecycleRecordsForNodeIn,
       AsyncCandidateLifecycleRecordsForIn,
       AsyncCandidateLifecycleRecordedIn,
       AsyncCandidateLifecycleRecordForIn,
       AsyncCandidateLifecycleClockRecordBucketIn,
       AsyncCandidateLifecycleOrdinaryRecordBucketIn,
       AsyncCandidateLifecycleClockOwnerCountIn,
       AsyncUnmaterializedTimeoutLifecycleReservationIn,
       AsyncFreshServeIngressAdmissionsForNodeThisStep,
       AsyncFreshServeIngressAdmissionsAreSingularThisStep,
       AsyncFreshServeIngressSchedulerReservationMatchesIn,
       AsyncFreshExactServeReservationThisStep,
       AsyncExactServeClockFreezeBoundaryThisStep,
       AsyncClockLifecycleFreezeBoundaryThisStep,
       AsyncCandidateProducerContinuationExactRuntimeReplayStep,
       AsyncRetransmitLifecycleCanAcquireThisStep,
       AsyncRetransmitLifecycleConsumesFreshOrdinal,
       AsyncRetransmitLifecycleFreshOrdinalForStep,
       AsyncRetransmitLifecyclePhysicalCutForStep,
       AsyncRetransmitLifecycleResetThisStep,
       AsyncRetransmitLifecycleEpisodeCompletesThisStep,
       AsyncRetransmitClockFreezeReady,
       AsyncRetransmitLifecycleOwned,
       AsyncRetransmitLifecycleOrdinal,
       AsyncRetransmitLifecyclePhysicalCut,
       RetransmitDue, RetransmitDueAfter, RetransmitTagPresent,
       AsyncTimeoutClockDue, AsyncTimeoutClockDueAfter,
       AsyncTimeoutClockDueIn, TimeoutTagPresentIn,
       ResponsiveReplayQuarantinedIn,
       TimeoutDue, TimeoutDueAfter,
       AsyncOlderRuntimeLifecycleBlocksTimeout,
       AsyncOlderRetransmitLifecycleBlocksTimeout,
       AsyncOlderCandidateLifecycleBlocksTimeout,
       AsyncOlderCandidateLifecycleBlocksRetransmit,
       AsyncRetransmitPriorityPrecedesCandidate,
       DirectRetransmitStep, DeferredRetransmitStep,
       AsyncTimeoutLifecycleNewOriginsForNodeIn,
       AsyncTimeoutLifecycleTransfersThisStep,
       AsyncTimeoutLifecycleResetThisStep,
       AsyncTimeoutLifecycleCanAcquireThisStep,
       AsyncTimeoutLifecycleUsesRecordedOriginOrdinal,
       AsyncTimeoutLifecycleOrdinalForStep,
       AsyncTimeoutLifecycleConsumesFreshOrdinal,
       AsyncCurrentTimeoutCausalOrigin,
       AsyncEffectiveTimeoutLifecycleOrigin,
       AsyncProposedTimeoutCausalOrigin,
       AsyncProposedTimeoutCausalCommand,
       TimeoutCausalCommand, AsyncTimeoutLifecycleOwned,
       NoItemCandidate, AsyncNoItemCandidateCausalOriginAt,
       AsyncCandidateWithIdentityAndOrigin,
       AsyncCandidateCausalOrigin,
       AsyncCandidateCausalOriginSet,
       NoAsyncCandidateLifecycleOrigin,
       AsyncOrderedScheduledCandidatesForNodeAfter,
       AsyncOrderedScheduledOriginsForNodeAfter,
       AsyncFirstScheduledOriginIndexAfter,
       AsyncControlServiceStateTypeInvariant,
       AsyncCandidateProducerContinuationLifecycleCoverageInvariant,
       AsyncCandidateProducerContinuationLifecycleCoverageInvariantIn,
       AsyncCandidateProducerContinuationLifecycleCoveredIn,
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
       AsyncCandidateServiceRecordRetainedAfterStep,
       AsyncCandidateServiceEligibleAfterStep,
       AsyncCandidateLifecyclePerNodeCapacityRespected,
       AsyncCandidateLifecycleAdmissionSet,
       AsyncCandidateLifecycleAdmission,
       AsyncCandidateLifecyclePhysicalCutInvariantIn,
       AsyncOrdinaryIngressCarrierPhysicalCutInvariantIn,
       AsyncOrdinaryIngressMinimumCarrierIn,
       AsyncOrdinaryIngressCarrierEvidenceOwnsOriginIn,
       AsyncDeferredOrdinaryIngressCarrierEvidenceForOriginIn,
       AsyncCandidateLifecycleSlots,
       AsyncCandidateLifecycleOrdinarySlots,
       AsyncCandidateLifecycleServicedSlots,
       AsyncCandidateLifecycleActiveSlots,
       AsyncCandidateLifecycleClockSlot,
       AsyncCandidateLifecycleAdmissions,
       AsyncNextCandidateLifecycleOrdinal,
       AsyncTimeoutLifecycleOrdinal,
       AsyncTimeoutLifecycleOrigin,
       AsyncControlServiceSlotSet,
       AsyncControlServiceSlot,
       AsyncControlServiceProtocolOwner,
       AsyncControlServiceAdmissionStartsOrReplaces,
       AsyncControlServiceCurrentHeightItem,
       AsyncLeaderWireServiceIdentity

(***************************************************************************
The candidate-lifecycle transformer types newly scheduled origins in the
post-state.  Its exact proof therefore consumes the primed scheduler type.
That fact is not assumed by the exported preservation theorem: it is derived
from the full unprimed strong invariant and the same `AsyncNext` step, then
threaded into the transformer above.
***************************************************************************)
THEOREM AsyncNextPreservesControlServiceStateTypeInvariant ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncNext
  => AsyncControlServiceStateTypeInvariant'
PROOF
  <1>1. ASSUME AsyncStrongTypeInvariant,
              AsyncNext
         PROVE AsyncControlServiceStateTypeInvariant'
    <2>1. /\ StrongInductiveInvariant
           /\ AsyncTypeInvariant
           /\ AsyncRecoveryTypeInvariant
           /\ AsyncControlServiceStateTypeInvariant
      BY <1>1, AsyncStrongTypeProjectsAsyncType
         DEF AsyncStrongTypeInvariant
    <2>2. AsyncSchedulerTypeInvariant'
      BY <1>1, <2>1, AsyncNextPreservesSchedulerType
    <2> QED BY <1>1, <2>1, <2>2,
         AsyncNextPreservesControlServiceStateTypeFromPrimedSchedulerType
  <1> QED BY <1>1

THEOREM AsyncNextPreservesLeaderWireIngressCarrierOwnership ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncNext
  => AsyncLeaderWireIngressCarrierOwnershipInvariant'
BY AdmitHiddenLeaderWireIsAtomicLocalAcceptanceCut,
   AdmitDormantLeaderWireRetainsLifecycleTokenAndFrozenPrefix,
   AtomicDormantLeaderWireAdmissionConsumesRealPacketWithFreshCarrier,
   AsyncUnboundChunkAdmissionDoesNotMintLeaderWireLifecycle,
   CoalescedDueLeaderWireLifecycleRetryPreservesFrozenOwner,
   LeaderWireIngressDrainNeverInventsRuntimeOwner,
   RuntimeLeaderWireCannotRetireMerelyFromIngressPop,
   RetireLeaderWireLifecycleRetainsTerminalTombstone,
   LeaderWireIgnoredOrServicedLastConsumerTerminalizesAtomically,
   AsyncDormantLeaderWireReactivationConsumesPhysicalNotLifecycleOrdinal,
   LeaderWireIngressAdmissionRefinesLifecycleTransition,
   LeaderWireIngressDrainRefinesLifecycleTransition,
   LeaderWireLastConsumerRefinesLifecycleTransition,
   LeaderWireTerminalRetirementRefinesLifecycleTransition,
   LeaderWireRestartReopenRefinesLifecycleTransition,
   FS_CardinalityType, FS_Subset, IsaT(7200)
   DEF AsyncStrongTypeInvariant,
       AsyncLeaderWireIngressCarrierOwnershipInvariant,
       AsyncLeaderWireIngressCarrierCoordinates,
       AsyncLeaderWireLifecycleIngressProtected,
       AsyncLeaderWireAdmissionMatchesRecord,
       AsyncLeaderWireLifecycleIdentityDerivable,
       AsyncChunkExactLifecycleCoordinatesRetained,
       AsyncLeaderWireLifecycleTransition,
       AsyncLeaderWireLifecycleIngressAdmissionTransition,
       AsyncLeaderWireLifecycleIngressDrainTransition,
       AsyncLeaderWireLifecycleConsumerTransition,
       AsyncLeaderWireLifecycleTerminalTransition,
       AsyncLeaderWireLifecycleRestartTransition,
       AsyncLeaderWireLifecycleStateAfterIngressAdmission,
       AsyncLeaderWireLifecycleRecordAfterIngressDrain,
       AsyncLeaderWireLifecyclesAfterIngressDrain,
       AsyncLeaderWireLifecycleStateAfterConsumerStep,
       AsyncLeaderWireLifecycleRecordAfterRestart,
       AsyncLeaderWireLifecyclesAfterRestart,
       RetireLeaderWireLifecycleSlot,
       AdmitIngressPacket, AdmitHiddenPacket, CoalesceHiddenPacket,
       DropPolicyRejectedHiddenPacket,
       PopSelectedIngress, DrainFairIngressSelected,
       DrainHistoricalIngressSelected,
       PreGstResponsiveRestart, PreGstResponsiveReplay,
       ResetNodeSchedulerForRestart,
       AsyncNext, AsyncNonCrashStep, AsyncRunnerStep,
       AsyncNonRunnerStep, AsyncNetworkStep, AsyncFaultStep,
       AsyncAllVars, AsyncSchedulerVars, IngressLane

THEOREM AsyncNextPreservesOrdinaryIngressCarrierOwnership ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncNext
  => AsyncOrdinaryIngressCarrierOwnershipInvariant'
BY ExactOrdinaryIngressDuplicateCoalescesWithoutCarrierAllocation,
   FS_CardinalityType, FS_Subset, IsaT(7200)
   DEF AsyncStrongTypeInvariant,
       AsyncOrdinaryIngressCarrierOwnershipInvariant,
       AsyncOrdinaryIngressCarrierCoordinates,
       AsyncControlServiceSlotTransition,
       AsyncOrdinaryIngressCarrierStateAfterTransition,
       AsyncOrdinaryIngressCarrierEvidenceAfterPhysicalTransition,
       AsyncOrdinaryIngressCarrierAfterPhysicalTransition,
       AsyncOrdinaryIngressCarrierStillPhysicalAfter,
       AsyncOrdinaryIngressCarrierRetainedAfterIn,
       AsyncCandidateLifecycleStateAfterOrdinaryIngressAdmission,
       AsyncFreshOrdinaryIngressCarrierEvidenceForNodeIn,
       AsyncFreshOrdinaryIngressCarrierItemsForNodeThisStep,
       AsyncOrdinaryIngressPhysicalAdmission,
       AsyncOrdinaryIngressCarrierItem,
       AsyncNext, AsyncNonCrashStep, AsyncRunnerStep,
       AsyncNonRunnerStep, AsyncNetworkStep, AsyncFaultStep,
       AdmitIngressPacket, AdmitHiddenPacket, CoalesceHiddenPacket,
       DropPolicyRejectedHiddenPacket,
       PopSelectedIngress, DrainFairIngressSelected,
       DrainHistoricalIngressSelected,
       PreGstResponsiveRestart, PreGstResponsiveReplay,
       ResetNodeSchedulerForRestart,
       AsyncAllVars, AsyncSchedulerVars, IngressLane

THEOREM AsyncNextPreservesCandidateLifecycleSchedulerCoverage ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncNext
  => AsyncCandidateLifecycleSchedulerCoverageInvariant'
BY AsyncNextNeverSchedulesAnUnownedCandidateLifecycle,
   FS_CardinalityType, FS_Subset, IsaT(7200)
   DEF AsyncStrongTypeInvariant,
       AsyncCandidateLifecycleSchedulerCoverageInvariant,
       AsyncCandidateLifecycleActiveRecords,
       AsyncCandidateLifecycleRecordCoversScheduledOrigin,
       AsyncScheduledCandidateOriginsForNode,
       AsyncCandidateLifecycleAdmissions,
       AsyncControlServiceSlotTransition,
       AsyncCandidateLifecycleStateAfterCarrierUpdate,
       AsyncCandidateLifecycleCarrierUpdatedAdmissions,
       AsyncCandidateLifecycleStateAfterOrdinaryIngressAdmission,
       AsyncCandidateLifecycleStateAfterServeIngressAdmission,
       AsyncCandidateLifecycleStateAfterCompaction,
       AsyncCandidateLifecycleStateAfterAdmission,
       AsyncCandidateLifecycleStateAfterTimeoutOwnership,
       AsyncCandidateLifecycleNewAdmissions,
       AsyncNewCandidateLifecycleOriginsForNodeIn,
       AsyncCandidateLifecycleOriginsRecordedForNodeIn,
       AsyncCandidateLifecycleRetirementCoveredIn,
       AsyncCandidateLifecycleDormantReservationOwnedAfter,
       AsyncCandidateLifecycleDeparturesThisStep,
       AsyncCandidateServicesThisStep,
       AsyncCandidateSemanticallyAppliedThisStep,
       AsyncCandidateSuccessfullyServicedThisStep,
       AsyncCandidateIgnoredWithoutApplicationThisStepSet,
       AsyncCandidateIgnoredWithoutApplicationThisStep,
       AsyncCandidatePhysicallyDiscardedThisStep,
       AsyncNext, AsyncNonCrashStep, AsyncRunnerStep,
       AsyncNonRunnerStep, AsyncNetworkStep, AsyncFaultStep,
       RunNode, RunHistoricalRecoveryNode, RunHistoricalServer,
       RunNodeWork, LocalAdmissionStep, IngressDrainStep,
       SerializedRuntimeStep, SerializedRuntimePrecedesServeIngressStep,
       SerializedLocalPrecedesServeIngressStep, RuntimeStep,
       FifoRuntimeStep, DeferredDrainStep,
       ServiceIoWorkerWork, AppendCausalSuccessors,
       AdmitIngressPacket, AdmitHiddenPacket, CoalesceHiddenPacket,
       DropPolicyRejectedHiddenPacket,
       PreGstResponsiveRestart, PreGstResponsiveReplay,
       ResetNodeSchedulerForRestart,
       CandidateScheduled, CandidateScheduledAfter,
       CandidateScheduledIn, EnqueueCandidate,
       AsyncAllVars, AsyncSchedulerVars

THEOREM AsyncNextPreservesServeProducerTurnTypeInvariant ==
  /\ AsyncServeProducerTurnTypeInvariant
  /\ AsyncNext
  => AsyncServeProducerTurnTypeInvariant'
PROOF
  <1>1. ASSUME AsyncServeProducerTurnTypeInvariant,
                AsyncNext
         PROVE AsyncServeProducerTurnTypeInvariant'
    <2>1. asyncServeProducerTurnReady
             \in [ValidatorIds -> BOOLEAN]
      BY <1>1 DEF AsyncServeProducerTurnTypeInvariant
    <2>2. AsyncServeProducerTurnTransition
      BY <1>1 DEF AsyncNext
    <2>3. \A node \in ValidatorIds:
             (IF AsyncServeProducerTurnCompletionStep(node)
              THEN TRUE
              ELSE IF AsyncServeProducerTurnAttemptThisStep(node)
                   THEN FALSE
                   ELSE asyncServeProducerTurnReady[node])
               \in BOOLEAN
      <3>1. ASSUME NEW node \in ValidatorIds
             PROVE (IF AsyncServeProducerTurnCompletionStep(node)
                    THEN TRUE
                    ELSE IF AsyncServeProducerTurnAttemptThisStep(node)
                         THEN FALSE
                         ELSE asyncServeProducerTurnReady[node])
                     \in BOOLEAN
        <4>1. asyncServeProducerTurnReady[node] \in BOOLEAN
          BY <2>1, <3>1, FunctionValueHasCodomain
        <4> QED BY <4>1, Isa
      <3> QED BY <3>1
    <2>4. [node \in ValidatorIds |->
             IF AsyncServeProducerTurnCompletionStep(node)
             THEN TRUE
             ELSE IF AsyncServeProducerTurnAttemptThisStep(node)
                  THEN FALSE
                  ELSE asyncServeProducerTurnReady[node]]
             \in [ValidatorIds -> BOOLEAN]
      BY <2>3, Isa
    <2>5. asyncServeProducerTurnReady'
             \in [ValidatorIds -> BOOLEAN]
      BY <2>2, <2>4
         DEF AsyncServeProducerTurnTransition
    <2> QED BY <2>5
         DEF AsyncServeProducerTurnTypeInvariant
  <1> QED BY <1>1

THEOREM AsyncNextPreservesServeProducerTurnInvariants ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncNext
  => /\ AsyncServeProducerTurnTypeInvariant'
     /\ AsyncServeProducerTurnOwnershipInvariant'
PROOF
  <1>1. ASSUME AsyncStrongTypeInvariant,
              AsyncNext
         PROVE /\ AsyncServeProducerTurnTypeInvariant'
               /\ AsyncServeProducerTurnOwnershipInvariant'
    <2>1. /\ StrongInductiveInvariant
           /\ AsyncTypeInvariant
           /\ AsyncRecoveryTypeInvariant
           /\ AsyncServeProducerTurnTypeInvariant
           /\ AsyncServeProducerTurnOwnershipInvariant
      BY <1>1, AsyncStrongTypeProjectsAsyncType
         DEF AsyncStrongTypeInvariant
    <2>2. AsyncServeProducerTurnTypeInvariant'
      BY <1>1, <2>1,
         AsyncNextPreservesServeProducerTurnTypeInvariant
    <2>3. AsyncSchedulerTypeInvariant'
      BY <1>1, <2>1, AsyncNextPreservesSchedulerType
    <2>4. \A node \in ValidatorIds:
             asyncServeProducerTurnReady'[node]
               => /\ AsyncServeIngressLifecycleOwnerIdentities(node)' = {}
                  /\ AsyncServeOffQueueReservations(node)' = {}
      <3>1. ASSUME NEW node \in ValidatorIds,
                    asyncServeProducerTurnReady'[node]
             PROVE /\ AsyncServeIngressLifecycleOwnerIdentities(node)' = {}
                   /\ AsyncServeOffQueueReservations(node)' = {}
        <4>1. CASE AsyncServeProducerTurnCompletionStep(node)
          BY <4>1
             DEF AsyncServeProducerTurnCompletionStep
        <4>2. CASE ~AsyncServeProducerTurnCompletionStep(node)
          <5>1. asyncServeProducerTurnReady[node]
            BY <1>1, <3>1, <4>2, Isa
               DEF AsyncNext, AsyncServeProducerTurnTransition
          <5>2. /\ AsyncServeIngressLifecycleOwnerIdentities(node) = {}
                 /\ AsyncServeOffQueueReservations(node) = {}
            BY <2>1, <3>1, <5>1
               DEF AsyncServeProducerTurnOwnershipInvariant
          <5>3. AsyncServeIngressLifecycleOwnerIdentities(node)' = {}
            BY <1>1, <2>1, <3>1, <5>1, <5>2,
               AsyncNextPreservesEmptyServeIngressOwnersWhileProducerTurnReady
          <5>4. AsyncServeOffQueueReservations(node)' = {}
            <6>1. ASSUME AsyncServeOffQueueReservations(node)' # {}
                   PROVE FALSE
              <7>1. PICK reservation
                       \in AsyncServeOffQueueReservations(node)': TRUE
                BY <6>1, FS_EmptySet, Zenon
              <7>2. AsyncServeIngressAdmissionOwned(
                       node, reservation.identity)'
                BY <2>3, <7>1
                   DEF AsyncSchedulerTypeInvariant,
                       AsyncIoTypeInvariant,
                       AsyncServeLifecycleTypeInvariant,
                       AsyncServeBarrierOwnsEarliestIngressOrdinalInvariant
              <7>3. PICK admission
                       \in AsyncServeIngressAdmissionRecords(
                            node, reservation.identity)': TRUE
                BY <7>2, FS_EmptySet, Zenon
                   DEF AsyncServeIngressAdmissionOwned
              <7>4. reservation.identity
                       \in AsyncServeIngressLifecycleOwnerIdentities(node)'
                BY <7>3, Isa
                   DEF AsyncServeIngressLifecycleOwnerIdentities,
                       AsyncServeIngressAdmissionIdentities,
                       AsyncServeIngressAdmissionRecords
              <7> QED BY <5>3, <7>4
            <6> QED BY <6>1
          <5> QED BY <5>3, <5>4
        <4> QED BY <4>1, <4>2
      <3> QED BY <3>1
    <2>5. AsyncServeProducerTurnOwnershipInvariant'
      BY <2>4 DEF AsyncServeProducerTurnOwnershipInvariant
    <2> QED BY <2>2, <2>5
  <1> QED BY <1>1

AsyncTimeoutRecoveryBoundaryFrameShape(episode) ==
  {"node", "key", "generation", "timeoutOwnerOrigin"}
    \subseteq DOMAIN episode

AsyncTimeoutRecoveryMutationFrameShape(episode) ==
  {"node", "key", "generation", "timeoutOwnerOrigin",
   "timeoutOwnerOrdinal", "physicalCut",
   "preFrozenRetransmitOrdinal", "preFrozenRetransmitPhysicalCut",
   "timeoutVoteOwnerUniverse", "admittedTimeoutVoteOwners"}
    \subseteq DOMAIN episode

THEOREM AsyncTimeoutRecoveryMutationFrameProjectsBoundaryFrame ==
  \A episode:
    AsyncTimeoutRecoveryMutationFrameShape(episode)
      => AsyncTimeoutRecoveryBoundaryFrameShape(episode)
BY Zenon
   DEF AsyncTimeoutRecoveryMutationFrameShape,
       AsyncTimeoutRecoveryBoundaryFrameShape

THEOREM AsyncTimeoutRecoveryEpisodeFromParametersHasMutationFrameShape ==
  \A parameters:
    AsyncTimeoutRecoveryMutationFrameShape(
      AsyncTimeoutRecoveryEpisodeFromParameters(parameters))
BY Isa
   DEF AsyncTimeoutRecoveryMutationFrameShape,
       AsyncTimeoutRecoveryEpisodeFromParameters,
       AsyncTimeoutRecoveryEpisode

THEOREM AsyncTimeoutRecoveryEpisodeSetHasMutationFrameShape ==
  \A episode \in AsyncTimeoutRecoveryEpisodeSet:
    AsyncTimeoutRecoveryMutationFrameShape(episode)
PROOF
  <1>1. ASSUME NEW episode \in AsyncTimeoutRecoveryEpisodeSet
         PROVE AsyncTimeoutRecoveryMutationFrameShape(episode)
    <2>1. PICK parameters \in AsyncTimeoutRecoveryEpisodeParameterSet:
             episode =
               AsyncTimeoutRecoveryEpisodeFromParameters(parameters)
      BY <1>1, Zenon DEF AsyncTimeoutRecoveryEpisodeSet
    <2>2. AsyncTimeoutRecoveryMutationFrameShape(
             AsyncTimeoutRecoveryEpisodeFromParameters(parameters))
      BY AsyncTimeoutRecoveryEpisodeFromParametersHasMutationFrameShape
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

AsyncTimeoutRecoveryEpisodeWithClearedFrozenPredecessor(episode) ==
  [[episode EXCEPT !.preFrozenRetransmitOrdinal = 0]
    EXCEPT !.preFrozenRetransmitPhysicalCut = 0]

THEOREM AsyncTimeoutRecoveryClearedFrozenPredecessorPreservesCurrentBoundary ==
  ASSUME NEW episode,
         AsyncTimeoutRecoveryMutationFrameShape(episode),
         AsyncTimeoutRecoveryEpisodeBoundaryIn(
           episode, context', nodeView', generation', decisions')
  PROVE /\ AsyncTimeoutRecoveryMutationFrameShape(
             AsyncTimeoutRecoveryEpisodeWithClearedFrozenPredecessor(
               episode))
        /\ AsyncTimeoutRecoveryEpisodeBoundaryIn(
             AsyncTimeoutRecoveryEpisodeWithClearedFrozenPredecessor(
               episode),
             context', nodeView', generation', decisions')
PROOF
  <1> DEFINE First ==
         [episode EXCEPT !.preFrozenRetransmitOrdinal = 0]
  <1> DEFINE Cleared ==
         [First EXCEPT !.preFrozenRetransmitPhysicalCut = 0]
  <1>1. {"node", "key", "generation", "timeoutOwnerOrigin",
           "timeoutOwnerOrdinal", "physicalCut",
           "preFrozenRetransmitOrdinal",
           "preFrozenRetransmitPhysicalCut",
           "timeoutVoteOwnerUniverse", "admittedTimeoutVoteOwners"}
           \subseteq DOMAIN episode
    BY DEF AsyncTimeoutRecoveryMutationFrameShape
  <1>1a. AsyncTimeoutRecoveryBoundaryFrameShape(episode)
    BY AsyncTimeoutRecoveryMutationFrameProjectsBoundaryFrame
  <1>2. DOMAIN First = DOMAIN episode
    BY <1>1, FunctionalReplacePreservesDomain DEF First
  <1>3. DOMAIN Cleared = DOMAIN First
    BY <1>1, <1>2, FunctionalReplacePreservesDomain DEF Cleared
  <1>4. AsyncTimeoutRecoveryMutationFrameShape(Cleared)
    BY <1>1, <1>2, <1>3
       DEF AsyncTimeoutRecoveryMutationFrameShape
  <1>5a. First.node = episode.node
    BY <1>1a, FunctionalUpdateAwayFromKey
       DEF First, AsyncTimeoutRecoveryBoundaryFrameShape
  <1>5b. First.key = episode.key
    BY <1>1a, FunctionalUpdateAwayFromKey
       DEF First, AsyncTimeoutRecoveryBoundaryFrameShape
  <1>5c. First.generation = episode.generation
    BY <1>1a, FunctionalUpdateAwayFromKey
       DEF First, AsyncTimeoutRecoveryBoundaryFrameShape
  <1>5d. First.timeoutOwnerOrigin = episode.timeoutOwnerOrigin
    BY <1>1a, FunctionalUpdateAwayFromKey
       DEF First, AsyncTimeoutRecoveryBoundaryFrameShape
  <1>6a. Cleared.node = First.node
    BY <1>1a, <1>2, FunctionalUpdateAwayFromKey
       DEF Cleared, AsyncTimeoutRecoveryBoundaryFrameShape
  <1>6b. Cleared.key = First.key
    BY <1>1a, <1>2, FunctionalUpdateAwayFromKey
       DEF Cleared, AsyncTimeoutRecoveryBoundaryFrameShape
  <1>6c. Cleared.generation = First.generation
    BY <1>1a, <1>2, FunctionalUpdateAwayFromKey
       DEF Cleared, AsyncTimeoutRecoveryBoundaryFrameShape
  <1>6d. Cleared.timeoutOwnerOrigin = First.timeoutOwnerOrigin
    BY <1>1a, <1>2, FunctionalUpdateAwayFromKey
       DEF Cleared, AsyncTimeoutRecoveryBoundaryFrameShape
  <1>7. AsyncTimeoutRecoveryEpisodeBoundaryIn(
           Cleared, context', nodeView', generation', decisions')
    BY <1>5a, <1>5b, <1>5c, <1>5d,
       <1>6a, <1>6b, <1>6c, <1>6d, Isa
       DEF AsyncTimeoutRecoveryEpisodeBoundaryIn
  <1>8. Cleared =
           AsyncTimeoutRecoveryEpisodeWithClearedFrozenPredecessor(
             episode)
    BY DEF First, Cleared,
           AsyncTimeoutRecoveryEpisodeWithClearedFrozenPredecessor
  <1> QED BY <1>4, <1>7, <1>8

=============================================================================
