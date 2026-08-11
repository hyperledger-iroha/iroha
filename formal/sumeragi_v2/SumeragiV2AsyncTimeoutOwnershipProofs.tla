---- MODULE SumeragiV2AsyncTimeoutOwnershipProofs ----
EXTENDS SumeragiV2AsyncDeadlockProofs

(***************************************************************************
Exact timeout/view liveness ownership.

The temporal proof below never identifies a timeout by a scheduler delivery
ordinal.  `TimeoutOrigin` retains the complete semantic vote record across
WAL, signing, and fan-out ownership, while `TimeoutDelivery` follows one
recipient's exact immutable network item.  A source which has installed a
higher timeout certificate retains a complete independent TC outbox; a source
which has already decided instead exposes the exact Commit certificate, or
after Apply the recurring historical Commit-certificate service authority.

The two finite cardinality ranks are deliberately source based.  Catch-up
counts responsive validators below the target round.  Receipt rank counts
responsive signer slots not yet recorded at the target.  Neither rank can be
reset by a duplicate delivery or by unrelated traffic.
***************************************************************************)

THEOREM RestartSignatureReplayIsNeverTombstoneSuppressed ==
  \A node:
    \A candidate \in SequenceSet(RestartSignatureReplay(node)):
      ~AsyncCandidateRestartReplayTombstoned(candidate)
BY RestartSignatureReplayCommandsAreSignatures,
   AsyncRestartScopedCandidateIsNeverReplayTombstoned

TimeoutVoteSemanticIdentity(node, roundView, vote) ==
  /\ vote \in TimeoutVoteRecordSet
  /\ vote.context = context
  /\ vote.height = height
  /\ vote.view = roundView
  /\ vote.signer = node

TimeoutVoteItem(vote, recipient) ==
  AsyncNetworkItem(
    "TimeoutVote", vote.signer, TimeoutEnvelope(recipient, vote))

TimeoutCertificateItem(source, recipient, tc) ==
  AsyncNetworkItem(
    "TimeoutCertificate", source, TcEnvelope(recipient, tc))

CommitCertificateItem(source, recipient, qc) ==
  AsyncNetworkItem("CommitQC", source, QcEnvelope(recipient, qc))

ExactPacketOwns(item) ==
  \E packet \in asyncTransport: packet.item = item

ExactIngressOwns(item) ==
  /\ item.envelope.recipient \in ValidatorIds
  /\ \E source \in AsyncIngressSources:
       item \in SequenceSet(
         IngressLane(item.envelope.recipient, source))

ExactDeliveryCandidateOwns(item) ==
  CandidateScheduled(DeliveryCandidate(item))

TimeoutDelivery(vote, recipient) ==
  LET item == TimeoutVoteItem(vote, recipient)
  IN /\ TimeoutVoteSemanticIdentity(vote.signer, vote.view, vote)
     /\ recipient \in CurrentVoters
     /\ \/ item \in asyncRetainedControl
        \/ ExactPacketOwns(item)
        \/ ExactIngressOwns(item)
        \/ ExactDeliveryCandidateOwns(item)

TimeoutReceipt(vote, recipient) ==
  TimeoutVoteAt(recipient, vote) \in receivedTimeoutVotes

TimeoutOrigin(node, roundView, vote) ==
  /\ TimeoutVoteSemanticIdentity(node, roundView, vote)
  /\ \/ TimeoutWal(node, vote) \in pendingTimeout
     \/ /\ vote \in timeoutIntents
           /\ TimeoutSign(node, vote) \in signTimeouts
     \/ /\ vote \in timeoutIntents
           /\ \A recipient \in CurrentVoters:
                TimeoutReceipt(vote, recipient)
                  \/ TimeoutDelivery(vote, recipient)

TimeoutRoundTrigger(node, roundView) ==
  /\ gst
  /\ node \in AsyncCurrentResponsiveVoters \cap up
  /\ nodeView[node] = roundView
  /\ ~NodeHasDecision(node)
  /\ ~NodeTimedOut(node, roundView)

TimeoutCertificateSemanticIdentity(tc, minimumView) ==
  /\ tc \in TcRecordSet
  /\ tc.context = context
  /\ tc.height = height
  /\ tc.view >= minimumView
  /\ TCValid(tc)

TimeoutCertificateDelivery(source, recipient, tc) ==
  LET item == TimeoutCertificateItem(source, recipient, tc)
  IN /\ source \in AsyncCurrentResponsiveVoters
     /\ recipient \in CurrentVoters
     /\ \/ item \in asyncRetainedControl
        \/ ExactPacketOwns(item)
        \/ ExactIngressOwns(item)
        \/ ExactDeliveryCandidateOwns(item)

TimeoutCertificateInstallOwner(recipient, tc) ==
  \/ TcAt(recipient, tc) \in receivedTCs
  \/ \E request \in pendingInstallTC:
       /\ request.node = recipient
       /\ request.tc = tc

TcFrontier(recipient, minimumView) ==
  \E source \in AsyncCurrentResponsiveVoters,
     tc \in TcRecordSet:
    /\ TimeoutCertificateSemanticIdentity(tc, minimumView)
    /\ \/ TimeoutCertificateDelivery(source, recipient, tc)
       \/ TimeoutCertificateInstallOwner(recipient, tc)

ResponsiveViewCertificateAuthority(source, minimumView) ==
  /\ source \in AsyncCurrentResponsiveVoters \cap up
  /\ ~NodeHasDecision(source)
  /\ \E tc \in TcRecordSet:
       /\ TimeoutCertificateSemanticIdentity(tc, minimumView)
       /\ nodeView[source] = tc.view + 1
       /\ tc = lastInstalledTc[source]
       /\ TcOutbox(source, tc) \subseteq asyncRetainedControl

DecisionSourceAt(source, qc) ==
  /\ qc \in QcRecordSet
  /\ qc.context = context
  /\ qc.height = height
  /\ qc.phase = "Commit"
  /\ [node |-> source, qc |-> qc] \in decisions

CommitCertificateDelivery(source, target, qc) ==
  LET item == CommitCertificateItem(source, target, qc)
  IN /\ source \in AsyncCurrentResponsiveVoters
     /\ target \in CurrentVoters
     /\ \/ item \in asyncRetainedControl
        \/ ExactPacketOwns(item)
        \/ ExactIngressOwns(item)
        \/ ExactDeliveryCandidateOwns(item)
        \/ QcAt(target, qc) \in receivedQCs
        \/ DecisionWal(target, qc, FALSE) \in pendingDecision

AppliedDecisionCertificateAuthority(source, qc) ==
  \E application \in applied:
    /\ application.node = source
    /\ application.qc = qc
    /\ application.qc.context = context
    /\ application.qc.phase = "Commit"

CommitCertificateRequestTo(target, source, request) ==
  /\ request \in AsyncNetworkItems
  /\ request.kind = "CommitCertificateRequest"
  /\ request.source = target
  /\ request.envelope.recipient = source
  /\ request.envelope.height = context.height

CommitCertificateResponseFor(target, source, qc, response) ==
  /\ response \in AsyncNetworkItems
  /\ response.kind = "CommitCertificateResponse"
  /\ response.source = source
  /\ response.envelope.recipient = target
  /\ response.envelope.qc = qc

CommitCertificateRoundTrip(target, source, qc) ==
  \/ \E request \in asyncActiveRequests:
       CommitCertificateRequestTo(target, source, request)
  \/ \E request \in AsyncNetworkItems:
       /\ CommitCertificateRequestTo(target, source, request)
       /\ \/ ExactPacketOwns(request)
          \/ ExactIngressOwns(request)
          \/ \E job \in AsyncServeJobSet:
               /\ job.candidate.item = request
               /\ ResponsiveProtectedServeJobOwned(source, job)
  \/ \E response \in AsyncNetworkItems:
       /\ CommitCertificateResponseFor(target, source, qc, response)
       /\ \/ ExactPacketOwns(response)
          \/ ExactIngressOwns(response)
          \/ CandidateScheduled(
               CommitCertificateResponseCandidate(response))
  \/ QcAt(target, qc) \in receivedQCs
  \/ DecisionWal(target, qc, FALSE) \in pendingDecision

DecisionPropagationFrontier(target) ==
  /\ target \in AsyncCurrentResponsiveVoters
  /\ ~NodeHasDecision(target)
  /\ \E source \in AsyncCurrentResponsiveVoters,
       qc \in QcRecordSet:
       /\ source # target
       /\ DecisionSourceAt(source, qc)
       /\ \/ CommitCertificateDelivery(source, target, qc)
          \/ AppliedDecisionCertificateAuthority(source, qc)
          \/ CommitCertificateRoundTrip(target, source, qc)

ResponsiveDecisionExists ==
  \E source \in AsyncCurrentResponsiveVoters:
    NodeHasDecision(source)

TimeoutMissingCatchupVoters(roundView) ==
  {node \in AsyncCurrentResponsiveVoters:
     nodeView[node] < roundView}

TimeoutMissingReceiptVoters(target, roundView) ==
  {signer \in AsyncCurrentResponsiveVoters:
     ~ReceivedTimeoutVoteAt(target, signer, roundView)}

TimeoutMissingCatchupRank(roundView) ==
  Cardinality(TimeoutMissingCatchupVoters(roundView))

TimeoutMissingReceiptRank(target, roundView) ==
  Cardinality(TimeoutMissingReceiptVoters(target, roundView))

TimeoutRoundStable(target, roundView) ==
  /\ gst
  /\ target \in AsyncCurrentResponsiveVoters \cap up
  /\ nodeView[target] = roundView
  /\ ~NodeHasDecision(target)

TimeoutDominatingFrontier(target, roundView) ==
  \/ DecisionPropagationFrontier(target)
  \/ \E source \in AsyncCurrentResponsiveVoters:
       ResponsiveViewCertificateAuthority(source, roundView)

TimeoutCatchupAtRank(target, roundView, rank) ==
  /\ TimeoutRoundStable(target, roundView)
  /\ ~ResponsiveDecisionExists
  /\ \A source \in AsyncCurrentResponsiveVoters:
       nodeView[source] <= roundView
  /\ TimeoutMissingCatchupRank(roundView) = rank

TimeoutReceiptAtRank(target, roundView, rank) ==
  /\ TimeoutRoundStable(target, roundView)
  /\ ~ResponsiveDecisionExists
  /\ \A source \in AsyncCurrentResponsiveVoters:
       nodeView[source] = roundView
  /\ TimeoutMissingReceiptRank(target, roundView) = rank

TimeoutSourceDominated(vote) ==
  \/ nodeView[vote.signer] > vote.view
  \/ NodeHasDecision(vote.signer)

TimeoutOriginOutcome(node, roundView, vote, recipient) ==
  \/ TimeoutReceipt(vote, recipient)
  \/ TimeoutDelivery(vote, recipient)
  \/ TimeoutSourceDominated(vote)
  \/ TimeoutViewGoal(recipient, roundView)
  \/ DecisionPropagationFrontier(recipient)

TimeoutDeliveryOutcome(vote, recipient) ==
  \/ TimeoutReceipt(vote, recipient)
  \/ TimeoutSourceDominated(vote)
  \/ TimeoutViewGoal(recipient, vote.view)
  \/ DecisionPropagationFrontier(recipient)

TcFrontierOutcome(recipient, minimumView) ==
  TimeoutViewGoal(recipient, minimumView)

DecisionPropagationOutcome(target) == NodeHasDecision(target)

TimeoutFormTcCandidateOwned(target, roundView) ==
  \E candidate \in AsyncCandidateSet:
    /\ candidate.node = target
    /\ candidate.height = context.height
    /\ candidate.view = roundView
    /\ candidate.kind = "FormTC"
    /\ CandidateScheduled(candidate)

TimeoutCertificateFormationFrontier(target, roundView) ==
  \/ TimeoutViewGoal(target, roundView)
  \/ TimeoutFormTcCandidateOwned(target, roundView)
  \/ TcFrontier(target, roundView)

TimeoutViewOwnershipInvariant ==
  /\ \A source \in AsyncCurrentResponsiveVoters:
       /\ gst
       /\ nodeView[source] > 0
       /\ ~NodeHasDecision(source)
       => ResponsiveViewCertificateAuthority(
            source, nodeView[source] - 1)
  /\ \A node \in AsyncCurrentResponsiveVoters,
       roundView \in Views, vote \in timeoutIntents:
       /\ gst
       /\ TimeoutVoteSemanticIdentity(node, roundView, vote)
       /\ nodeView[node] = roundView
       /\ ~NodeHasDecision(node)
       => TimeoutOrigin(node, roundView, vote)
  /\ \A target \in AsyncCurrentResponsiveVoters,
       roundView \in Views:
       /\ gst
       /\ nodeView[target] = roundView
       /\ ~NodeHasDecision(target)
       /\ ResponsiveTimeoutReceiptQuorumAt(target, roundView)
       => TimeoutCertificateFormationFrontier(target, roundView)
  /\ \A source, target \in AsyncCurrentResponsiveVoters,
       qc \in QcRecordSet:
       /\ source # target
       /\ DecisionSourceAt(source, qc)
       /\ ~NodeHasDecision(target)
       => DecisionPropagationFrontier(target)

(***************************************************************************
Inductive ownership kernel.

The public invariant above names transport and scheduler frontiers.  The
transition induction is clearer over the four durable authorities which
create those frontiers:

  * an installed view retains the exact preceding TC broadcast batch;
  * a current timeout intent is in WAL, signing, retained broadcast, or the
    one pre-GST responsive replay lifecycle;
  * a completed responsive receipt quorum owns its exact local install WAL;
    and
  * every current-context Decision retains its exact CommitQC broadcast.

The recovery disjunct is deliberately unavailable once replay starts.  It
covers only `RestartRequired` and `ReplayRequired`, after a responsive
pre-GST crash has removed volatile timeout staging and before the replay entry
action reconstructs the exact timeout signature request.  `Replaying` and
`Recovered` therefore already require the concrete WAL/sign/retained owner;
the GST projection does not hide an unfinished replay lifecycle.
***************************************************************************)

RetainedViewCertificateAuthority(source, minimumView) ==
  /\ source \in AsyncCurrentResponsiveVoters
  /\ ~NodeHasDecision(source)
  /\ \E tc \in TcRecordSet:
       /\ TimeoutCertificateSemanticIdentity(tc, minimumView)
       /\ nodeView[source] = tc.view + 1
       /\ tc = lastInstalledTc[source]
       /\ TcOutbox(source, tc) \subseteq asyncRetainedControl

TimeoutReplayRecoveryAuthority(node) ==
  /\ asyncRecoveryPhase
       \in {"RestartRequired", "ReplayRequired"}
  /\ asyncRecoveryNode = node
  /\ generation[node] = asyncRecoveryGeneration

RetainedTimeoutVoteAuthority(node, vote) ==
  TimeoutOutbox(TimeoutSign(node, vote))
    \subseteq asyncRetainedControl

TimeoutVoteConcreteAuthority(node, vote) ==
  \/ TimeoutWal(node, vote) \in pendingTimeout
  \/ TimeoutSign(node, vote) \in signTimeouts
  \/ RetainedTimeoutVoteAuthority(node, vote)

TimeoutVoteLifecycleAuthority(node, vote) ==
  \/ TimeoutVoteConcreteAuthority(node, vote)
  \/ TimeoutReplayRecoveryAuthority(node)

TimeoutReceiptQuorumInstallAuthority(target, roundView) ==
  \E tc \in TcRecordSet:
    /\ TimeoutCertificateSemanticIdentity(tc, roundView)
    /\ tc.view = roundView
    /\ InstallTcWal(target, tc, TRUE) \in pendingInstallTC

DecisionCertificateRetainedAuthority(source, qc) ==
  QcOutbox(source, qc) \subseteq asyncRetainedControl

ResponsiveRetainedTimeoutVoteControlSound ==
  \A item \in asyncRetainedControl:
    /\ item.kind = "TimeoutVote"
    /\ item.source \in AsyncCurrentResponsiveVoters
    => /\ item.envelope.vote.signer = item.source
       /\ item.envelope.vote.context = context
       /\ item.envelope.vote \in timeoutIntents
       /\ item.envelope.vote.view <= nodeView[item.source]

ResponsiveRetainedTcControlSound ==
  \A item \in asyncRetainedControl:
    /\ item.kind = "TimeoutCertificate"
    /\ item.source \in AsyncCurrentResponsiveVoters
    => /\ item.envelope.tc.context = context
       /\ TCValid(item.envelope.tc)
       /\ item.envelope.tc.view + 1 <= nodeView[item.source]

ResponsiveRetainedDecisionControlSound ==
  \A item \in asyncRetainedControl:
    /\ item.kind = "CommitQC"
    /\ item.source \in AsyncCurrentResponsiveVoters
    => /\ item.envelope.qc.context = context
       /\ item.envelope.qc.phase = "Commit"
       /\ item.envelope.qc \in commitQCs
       /\ item.envelope.qc.view <= nodeView[item.source]

ResponsiveRetainedTimeoutOwnershipControlSound ==
  /\ ResponsiveRetainedTimeoutVoteControlSound
  /\ ResponsiveRetainedTcControlSound
  /\ ResponsiveRetainedDecisionControlSound

ResponsiveInstalledTcAuthorityInvariant ==
  \A source \in AsyncCurrentResponsiveVoters:
       /\ nodeView[source] > 0
       /\ ~NodeHasDecision(source)
       => RetainedViewCertificateAuthority(
            source, nodeView[source] - 1)

ResponsiveTimeoutVoteAuthorityInvariant ==
  \A node \in AsyncCurrentResponsiveVoters,
       roundView \in Views, vote \in timeoutIntents:
       /\ TimeoutVoteSemanticIdentity(node, roundView, vote)
       /\ nodeView[node] = roundView
       /\ ~NodeHasDecision(node)
       => TimeoutVoteLifecycleAuthority(node, vote)

ResponsiveTimeoutQuorumAuthorityInvariant ==
  \A target \in AsyncCurrentResponsiveVoters,
       roundView \in Views:
       /\ nodeView[target] = roundView
       /\ ~NodeHasDecision(target)
       /\ ResponsiveTimeoutReceiptQuorumAt(target, roundView)
       => TimeoutReceiptQuorumInstallAuthority(target, roundView)

ResponsiveDecisionCertificateAuthorityInvariant ==
  \A source \in AsyncCurrentResponsiveVoters,
       qc \in QcRecordSet:
       DecisionSourceAt(source, qc)
         => DecisionCertificateRetainedAuthority(source, qc)

TimeoutViewOwnershipKernelInvariant ==
  /\ ResponsiveRetainedTimeoutOwnershipControlSound
  /\ ResponsiveInstalledTcAuthorityInvariant
  /\ ResponsiveTimeoutVoteAuthorityInvariant
  /\ ResponsiveTimeoutQuorumAuthorityInvariant
  /\ ResponsiveDecisionCertificateAuthorityInvariant

THEOREM TimeoutViewOwnershipKernelProjectsPublicInvariant ==
  /\ AsyncStrongTypeInvariant
  /\ TimeoutViewOwnershipKernelInvariant
  => TimeoutViewOwnershipInvariant
BY GstResponsiveNodesAreUp, IsaT(300)
   DEF AsyncStrongTypeInvariant, AsyncGstRecoveryPhaseInvariant,
       TimeoutViewOwnershipKernelInvariant,
       ResponsiveRetainedTimeoutOwnershipControlSound,
       ResponsiveRetainedTimeoutVoteControlSound,
       ResponsiveRetainedTcControlSound,
       ResponsiveRetainedDecisionControlSound,
       ResponsiveInstalledTcAuthorityInvariant,
       ResponsiveTimeoutVoteAuthorityInvariant,
       ResponsiveTimeoutQuorumAuthorityInvariant,
       ResponsiveDecisionCertificateAuthorityInvariant,
       RetainedViewCertificateAuthority,
       TimeoutVoteLifecycleAuthority,
       TimeoutVoteConcreteAuthority,
       RetainedTimeoutVoteAuthority,
       TimeoutReplayRecoveryAuthority,
       TimeoutReceiptQuorumInstallAuthority,
       DecisionCertificateRetainedAuthority,
       TimeoutViewOwnershipInvariant,
       ResponsiveViewCertificateAuthority,
       TimeoutOrigin, TimeoutDelivery,
       TimeoutVoteItem, TimeoutVoteSemanticIdentity,
       TimeoutCertificateFormationFrontier,
       TcFrontier, TimeoutCertificateInstallOwner,
       TimeoutCertificateSemanticIdentity,
       DecisionPropagationFrontier,
       CommitCertificateDelivery, CommitCertificateItem,
       DecisionSourceAt

THEOREM AsyncInitEstablishesTimeoutViewOwnershipKernel ==
  \A initialContext:
    AsyncInitAt(initialContext)
      => TimeoutViewOwnershipKernelInvariant
BY IsaT(300)
   DEF AsyncInitAt, AsyncBaseInitAt, InitAt,
       TimeoutViewOwnershipKernelInvariant,
       ResponsiveRetainedTimeoutOwnershipControlSound,
       ResponsiveRetainedTimeoutVoteControlSound,
       ResponsiveRetainedTcControlSound,
       ResponsiveRetainedDecisionControlSound,
       ResponsiveInstalledTcAuthorityInvariant,
       ResponsiveTimeoutVoteAuthorityInvariant,
       ResponsiveTimeoutQuorumAuthorityInvariant,
       ResponsiveDecisionCertificateAuthorityInvariant,
       RetainedViewCertificateAuthority,
       TimeoutVoteLifecycleAuthority,
       TimeoutVoteConcreteAuthority,
       RetainedTimeoutVoteAuthority,
       TimeoutReplayRecoveryAuthority,
       TimeoutReceiptQuorumInstallAuthority,
       DecisionCertificateRetainedAuthority,
       TimeoutVoteSemanticIdentity,
       TimeoutCertificateSemanticIdentity,
       DecisionSourceAt, NodeHasDecision,
       ResponsiveTimeoutReceiptQuorumAt,
       AsyncCurrentResponsiveVoters,
       AsyncRuntimeInit, AsyncTransportInit

TimeoutOwnershipControlKinds ==
  {"TimeoutVote", "TimeoutCertificate", "CommitQC"}

TimeoutOwnershipRetainedItemsIn(retained) ==
  {item \in retained: item.kind \in TimeoutOwnershipControlKinds}

TimeoutOwnershipRetainedItems ==
  TimeoutOwnershipRetainedItemsIn(asyncRetainedControl)

TimeoutViewOwnershipKernelProjection ==
  <<context, nodeView, generation, decisions, timeoutIntents,
    pendingTimeout, signTimeouts, receivedTimeoutVotes, pendingInstallTC,
    commitQCs, lastInstalledTc,
    TimeoutOwnershipRetainedItems, AsyncRecoveryControlVars>>

THEOREM TimeoutViewOwnershipKernelProjectionFrame ==
  /\ TimeoutViewOwnershipKernelInvariant
  /\ UNCHANGED TimeoutViewOwnershipKernelProjection
  => TimeoutViewOwnershipKernelInvariant'
BY Isa
   DEF TimeoutViewOwnershipKernelInvariant,
       ResponsiveRetainedTimeoutOwnershipControlSound,
       ResponsiveRetainedTimeoutVoteControlSound,
       ResponsiveRetainedTcControlSound,
       ResponsiveRetainedDecisionControlSound,
       ResponsiveInstalledTcAuthorityInvariant,
       ResponsiveTimeoutVoteAuthorityInvariant,
       ResponsiveTimeoutQuorumAuthorityInvariant,
       ResponsiveDecisionCertificateAuthorityInvariant,
       RetainedViewCertificateAuthority,
       TimeoutVoteLifecycleAuthority,
       TimeoutVoteConcreteAuthority,
       RetainedTimeoutVoteAuthority,
       TimeoutReplayRecoveryAuthority,
       TimeoutReceiptQuorumInstallAuthority,
       DecisionCertificateRetainedAuthority,
       TimeoutVoteSemanticIdentity,
       TimeoutCertificateSemanticIdentity,
       DecisionSourceAt, NodeHasDecision,
       ResponsiveTimeoutReceiptQuorumAt,
       TimeoutOwnershipRetainedItems,
       TimeoutOwnershipRetainedItemsIn,
       TimeoutOwnershipControlKinds,
       TimeoutViewOwnershipKernelProjection,
       AsyncRecoveryControlVars,
       AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch

THEOREM RememberedUnownedControlPreservesTimeoutOwnershipItems ==
  \A retained, items:
    (\A item \in items:
       item.kind \notin TimeoutOwnershipControlKinds)
      => TimeoutOwnershipRetainedItemsIn(
           RememberedControl(retained, items))
           = TimeoutOwnershipRetainedItemsIn(retained)
BY IsaM("blast")
   DEF TimeoutOwnershipRetainedItemsIn,
       TimeoutOwnershipControlKinds,
       RememberedControl, RetainedClassItems, ControlClass

THEOREM RememberExactInstalledTcAfterRemovingClass ==
  \A retained, node, tc:
    TcOutbox(node, tc)
      \subseteq RememberedControl(
        retained \ RetainedClassItems(
                     retained, node, "TimeoutCertificate"),
        TcOutbox(node, tc))
BY IsaM("blast")
   DEF TcOutbox, RememberedControl,
       RetainedClassItems, ControlClass, ControlView

THEOREM ExecuteRegularCommandPreservesTimeoutViewOwnershipKernel ==
  \A command:
    /\ AsyncStrongTypeInvariant
    /\ TimeoutViewOwnershipKernelInvariant
    /\ ExecuteRegularCommand(command)
    /\ UNCHANGED AsyncRecoveryControlVars
    => TimeoutViewOwnershipKernelInvariant'
BY IsaM("blast")
   DEF AsyncStrongTypeInvariant,
       StrongInductiveInvariant, Safety, TypeInvariant,
       TimeoutViewOwnershipKernelInvariant,
       ResponsiveRetainedTimeoutOwnershipControlSound,
       ResponsiveRetainedTimeoutVoteControlSound,
       ResponsiveRetainedTcControlSound,
       ResponsiveRetainedDecisionControlSound,
       ResponsiveInstalledTcAuthorityInvariant,
       ResponsiveTimeoutVoteAuthorityInvariant,
       ResponsiveTimeoutQuorumAuthorityInvariant,
       ResponsiveDecisionCertificateAuthorityInvariant,
       RetainedViewCertificateAuthority,
       TimeoutVoteLifecycleAuthority,
       TimeoutVoteConcreteAuthority,
       RetainedTimeoutVoteAuthority,
       TimeoutReplayRecoveryAuthority,
       TimeoutReceiptQuorumInstallAuthority,
       DecisionCertificateRetainedAuthority,
       TimeoutVoteSemanticIdentity,
       TimeoutCertificateSemanticIdentity,
       DecisionSourceAt, NodeHasDecision,
       ResponsiveTimeoutReceiptQuorumAt, ReceivedTimeoutVoteAt,
       ExecuteRegularCommand, RegularCoreCommand,
       AssembleLocalBody, BeginLocalProposal, PersistProposal,
       FetchBody, RebindRetainedBody, StoreBody,
       ValidateBody, ValidateDecidedBody, ValidateLockedBody, RejectBody,
       BeginPrepare, PersistPrepare,
       BeginObservePrepare, PersistObservePrepare,
       BeginLockCommit, PersistLockCommit,
       FormCommitQC, BeginDecision, PersistTimeout, BeginInstallTC,
       AcceptCertifiedResponseCapability,
       RetireCompletedBodyCertifiedResponseAuthority,
       TimeoutOwnershipRetainedItems,
       TimeoutOwnershipRetainedItemsIn,
       TimeoutOwnershipControlKinds,
       AsyncRecoveryControlVars, AsyncAuxVars, vars,
       AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch

THEOREM ExecuteSignTimeoutPreservesTimeoutViewOwnershipKernel ==
  \A command:
    /\ AsyncStrongTypeInvariant
    /\ TimeoutViewOwnershipKernelInvariant
    /\ ExecuteSignTimeout(command)
    /\ UNCHANGED AsyncRecoveryControlVars
    => TimeoutViewOwnershipKernelInvariant'
BY IsaM("blast")
   DEF AsyncStrongTypeInvariant,
       StrongInductiveInvariant, Safety, TypeInvariant,
       ReducerProvenanceInvariant, HonestTimeoutTransportBacked,
       AsyncSerializedBusyKernelInvariant, AsyncBusyReadinessInvariant,
       TimeoutViewOwnershipKernelInvariant,
       ResponsiveRetainedTimeoutOwnershipControlSound,
       ResponsiveRetainedTimeoutVoteControlSound,
       ResponsiveRetainedTcControlSound,
       ResponsiveRetainedDecisionControlSound,
       ResponsiveInstalledTcAuthorityInvariant,
       ResponsiveTimeoutVoteAuthorityInvariant,
       ResponsiveTimeoutQuorumAuthorityInvariant,
       ResponsiveDecisionCertificateAuthorityInvariant,
       RetainedViewCertificateAuthority,
       TimeoutVoteLifecycleAuthority,
       TimeoutVoteConcreteAuthority,
       RetainedTimeoutVoteAuthority,
       TimeoutReplayRecoveryAuthority,
       TimeoutReceiptQuorumInstallAuthority,
       DecisionCertificateRetainedAuthority,
       TimeoutVoteSemanticIdentity,
       TimeoutCertificateSemanticIdentity,
       DecisionSourceAt, NodeHasDecision,
       ResponsiveTimeoutReceiptQuorumAt, ReceivedTimeoutVoteAt,
       ExecuteSignTimeout, CompleteTimeoutSignature,
       LocalTimeoutCompletionGuard, LocalTimeoutVoteFor,
       TimeoutReceiptsAfter, TimeoutReceiptAdmitted,
       TimeoutVoteSlotOccupied, SameTimeoutVoteSlot,
       TimeoutCertificateAfterReceipt,
       TimeoutInstallRequestAfterReceipt, TimeoutReceiptFormsTC,
       PublishControlItems, TimeoutOutbox,
       RememberedControl, RetainedClassItems, ControlClass, ControlView,
       PacketForItem, PacketsForItems,
       TimeoutOwnershipRetainedItems,
       TimeoutOwnershipRetainedItemsIn,
       TimeoutOwnershipControlKinds,
       AsyncRecoveryControlVars, vars,
       AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch

THEOREM ExecutePersistInstallRetainsExactInstalledTcAuthority ==
  \A command:
    ExecutePersistInstall(command)
      => \E request \in pendingInstallTC:
           /\ command.node = request.node
           /\ command.view = request.tc.view
           /\ InstallTcEvidenceMatches(command, request.tc)
           /\ lastInstalledTc'[request.node] = request.tc
           /\ TcOutbox(request.node, request.tc)
                \subseteq asyncRetainedControl'
BY RememberExactInstalledTcAfterRemovingClass, IsaM("blast")
   DEF ExecutePersistInstall, PersistInstallTC,
       PersistInstalledControlAfterInstall,
       InstalledControlAfterTC, CurrentTimeoutControlFor,
       ReseedExactHighestPrepareControl,
       RememberedControl, RetainedClassItems,
       ControlClass, ControlView, TcOutbox

THEOREM ExecutePersistInstallPreservesTimeoutViewOwnershipKernel ==
  \A command:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ DecisionTimeoutFrontierInvariant
    /\ TimeoutViewOwnershipKernelInvariant
    /\ ExecutePersistInstall(command)
    /\ UNCHANGED AsyncRecoveryControlVars
    => TimeoutViewOwnershipKernelInvariant'
BY ExecutePersistInstallRetainsExactInstalledTcAuthority,
   IsaM("blast")
   DEF AsyncStrongTypeInvariant,
       StrongInductiveInvariant, Safety, TypeInvariant,
       ReducerProvenanceInvariant, PendingCertificateWritesAuthorized,
       AsyncProgressOwnershipInvariant,
       SerializedBusyOwnershipInvariant,
       DecisionTimeoutFrontierInvariant,
       PendingTimeoutExcludesDecision,
       PendingInstallExcludesDecision,
       TimeoutSigningExcludesDecision,
       PendingDecisionExcludesTimeoutWork,
       TimeoutViewOwnershipKernelInvariant,
       ResponsiveRetainedTimeoutOwnershipControlSound,
       ResponsiveRetainedTimeoutVoteControlSound,
       ResponsiveRetainedTcControlSound,
       ResponsiveRetainedDecisionControlSound,
       ResponsiveInstalledTcAuthorityInvariant,
       ResponsiveTimeoutVoteAuthorityInvariant,
       ResponsiveTimeoutQuorumAuthorityInvariant,
       ResponsiveDecisionCertificateAuthorityInvariant,
       RetainedViewCertificateAuthority,
       TimeoutVoteLifecycleAuthority,
       TimeoutVoteConcreteAuthority,
       RetainedTimeoutVoteAuthority,
       TimeoutReplayRecoveryAuthority,
       TimeoutReceiptQuorumInstallAuthority,
       DecisionCertificateRetainedAuthority,
       TimeoutVoteSemanticIdentity,
       TimeoutCertificateSemanticIdentity,
       DecisionSourceAt, NodeHasDecision,
       ResponsiveTimeoutReceiptQuorumAt, ReceivedTimeoutVoteAt,
       ExecutePersistInstall, PersistInstallTC,
       PersistInstalledControlAfterInstall,
       InstalledControlAfterTC, CurrentTimeoutControlFor,
       ReseedExactHighestPrepareControl,
       RememberedControl, RetainedClassItems, ControlClass, ControlView,
       TcOutbox, QcOutbox, PacketForItem, PacketsForItems,
       StrictSameRoundTcUpgrade, TimeoutReceiptSurvivesInstall,
       TimeoutOwnershipRetainedItems,
       TimeoutOwnershipRetainedItemsIn,
       TimeoutOwnershipControlKinds,
       AsyncRecoveryControlVars, vars,
       AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch,
       RequestNodeSet, AllPendingRequests, NoDecisionForNode

THEOREM ExecutePersistDecisionPreservesTimeoutViewOwnershipKernel ==
  \A command:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ DecisionFrontierUniquenessInvariant
    /\ DecisionTimeoutFrontierInvariant
    /\ TimeoutViewOwnershipKernelInvariant
    /\ ExecutePersistDecision(command)
    /\ UNCHANGED AsyncRecoveryControlVars
    => TimeoutViewOwnershipKernelInvariant'
BY IsaM("blast")
   DEF AsyncStrongTypeInvariant,
       StrongInductiveInvariant, Safety, TypeInvariant,
       ReducerProvenanceInvariant, PendingCertificateWritesAuthorized,
       AsyncProgressOwnershipInvariant,
       SerializedBusyOwnershipInvariant,
       DecisionFrontierUniquenessInvariant,
       DecisionsUniqueByNodeContext,
       DecisionTimeoutFrontierInvariant,
       PendingTimeoutExcludesDecision,
       PendingInstallExcludesDecision,
       TimeoutSigningExcludesDecision,
       PendingDecisionExcludesTimeoutWork,
       TimeoutViewOwnershipKernelInvariant,
       ResponsiveRetainedTimeoutOwnershipControlSound,
       ResponsiveRetainedTimeoutVoteControlSound,
       ResponsiveRetainedTcControlSound,
       ResponsiveRetainedDecisionControlSound,
       ResponsiveInstalledTcAuthorityInvariant,
       ResponsiveTimeoutVoteAuthorityInvariant,
       ResponsiveTimeoutQuorumAuthorityInvariant,
       ResponsiveDecisionCertificateAuthorityInvariant,
       RetainedViewCertificateAuthority,
       TimeoutVoteLifecycleAuthority,
       TimeoutVoteConcreteAuthority,
       RetainedTimeoutVoteAuthority,
       TimeoutReplayRecoveryAuthority,
       TimeoutReceiptQuorumInstallAuthority,
       DecisionCertificateRetainedAuthority,
       TimeoutVoteSemanticIdentity,
       TimeoutCertificateSemanticIdentity,
       DecisionSourceAt, NodeHasDecision,
       ResponsiveTimeoutReceiptQuorumAt, ReceivedTimeoutVoteAt,
       ExecutePersistDecision, PersistDecision,
       PersistDecisionControl, QcOutbox,
       RememberedControl, RetainedClassItems, ControlClass, ControlView,
       PacketForItem, PacketsForItems,
       TimeoutOwnershipRetainedItems,
       TimeoutOwnershipRetainedItemsIn,
       TimeoutOwnershipControlKinds,
       AsyncRecoveryControlVars, vars,
       AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch,
       RequestNodeSet, AllPendingRequests, NoDecisionForNode

THEOREM ExecuteCoreDeliveryPreservesTimeoutViewOwnershipKernel ==
  \A command:
    /\ AsyncStrongTypeInvariant
    /\ TimeoutViewOwnershipKernelInvariant
    /\ ExecuteCoreDelivery(command)
    /\ UNCHANGED AsyncRecoveryControlVars
    => TimeoutViewOwnershipKernelInvariant'
BY RememberedUnownedControlPreservesTimeoutOwnershipItems,
   IsaM("blast")
   DEF AsyncStrongTypeInvariant,
       StrongInductiveInvariant, Safety, TypeInvariant,
       ReducerProvenanceInvariant,
       TimeoutViewOwnershipKernelInvariant,
       ResponsiveRetainedTimeoutOwnershipControlSound,
       ResponsiveRetainedTimeoutVoteControlSound,
       ResponsiveRetainedTcControlSound,
       ResponsiveRetainedDecisionControlSound,
       ResponsiveInstalledTcAuthorityInvariant,
       ResponsiveTimeoutVoteAuthorityInvariant,
       ResponsiveTimeoutQuorumAuthorityInvariant,
       ResponsiveDecisionCertificateAuthorityInvariant,
       RetainedViewCertificateAuthority,
       TimeoutVoteLifecycleAuthority,
       TimeoutVoteConcreteAuthority,
       RetainedTimeoutVoteAuthority,
       TimeoutReplayRecoveryAuthority,
       TimeoutReceiptQuorumInstallAuthority,
       DecisionCertificateRetainedAuthority,
       TimeoutVoteSemanticIdentity,
       TimeoutCertificateSemanticIdentity,
       DecisionSourceAt, NodeHasDecision,
       ResponsiveTimeoutReceiptQuorumAt, ReceivedTimeoutVoteAt,
       ExecuteCoreDelivery,
       DeliverProposal, DeliverVote, DeliverQC, DeliverTimeout, DeliverTC,
       TimeoutDeliveryGuard,
       TimeoutReceiptsAfter, TimeoutReceiptAdmitted,
       TimeoutVoteSlotOccupied, SameTimeoutVoteSlot,
       TimeoutCertificateAfterReceipt,
       TimeoutInstallRequestAfterReceipt, TimeoutReceiptFormsTC,
       RememberedControl, RetainedClassItems, ControlClass, ControlView,
       QcOutbox, TimeoutOwnershipRetainedItems,
       TimeoutOwnershipRetainedItemsIn,
       TimeoutOwnershipControlKinds,
       AsyncRecoveryControlVars, vars,
       AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch

ExactTimeoutInstallCandidate(command) ==
  CausalCandidateWithEvidence(
    "Completion", "PersistInstallTC", command,
    ExactFormedTcForTimeoutCommand(command))

THEOREM TimeoutFormingCommandCreatesExactInstallAuthority ==
  \A command:
    /\ (\/ /\ ExecuteSignTimeout(command)
             /\ SignTimeoutFormsTC(command)
        \/ /\ ExecuteCoreDelivery(command)
             /\ DeliverTimeoutFormsTC(command))
    /\ AppendCausalSuccessors(command)
    => LET tc == ExactFormedTcForTimeoutCommand(command)
           candidate == ExactTimeoutInstallCandidate(command)
       IN /\ TimeoutCertificateSemanticIdentity(tc, command.view)
          /\ InstallTcWal(command.node, tc, TRUE)
               \in pendingInstallTC'
          /\ candidate.kind = "PersistInstallTC"
          /\ candidate.evidence = tc
          /\ InstallTcEvidenceMatches(candidate, tc)
          /\ \/ CandidateScheduledAfter(candidate)
             \/ CandidateAdmissionCoalesced(candidate)
BY IsaT(1200)
   DEF ExecuteSignTimeout, ExecuteCoreDelivery,
       CompleteTimeoutSignature, DeliverTimeout,
       SignTimeoutFormsTC, SignTimeoutRequests,
       DeliverTimeoutFormsTC,
       ExactFormedTcForTimeoutCommand,
       ExactTimeoutInstallCandidate,
       CausalCandidateWithEvidence,
       AsyncCandidateWithIdentityAndOrigin,
       CommandSuccessors, FreshCommandSuccessors,
       FreshCandidateSequence, AppendCausalSuccessors,
       CandidateScheduledAfter, CandidateScheduledIn,
       CandidateAdmissionCoalesced,
       TimeoutCertificateSemanticIdentity,
       TimeoutCertificateAfterReceipt,
       TimeoutInstallRequestAfterReceipt,
       TimeoutReceiptFormsTC, TimeoutReceiptsAfter,
       TimeoutReceiptAdmitted, TimeoutVoteSlotOccupied,
       SameTimeoutVoteSlot, LocalTimeoutCompletionGuard,
       LocalTimeoutVoteFor, InstallTcEvidenceMatches,
       SequenceSet

THEOREM ExecuteOtherCommandLeavesTimeoutOwnershipProjection ==
  \A command:
    /\ ~ExecuteRegularCommand(command)
    /\ ~ExecuteSignTimeout(command)
    /\ ~ExecutePersistInstall(command)
    /\ ~ExecutePersistDecision(command)
    /\ ~ExecuteCoreDelivery(command)
    /\ ExecuteCommand(command)
    /\ UNCHANGED AsyncRecoveryControlVars
    => UNCHANGED TimeoutViewOwnershipKernelProjection
BY RememberedUnownedControlPreservesTimeoutOwnershipItems,
   IsaM("blast")
   DEF ExecuteCommand, ExecuteDecisionFetch,
       ExecuteSignProposal, ExecuteSignVote, ExecuteFormPrepareQC,
       ExecuteRequestCertifiedBody, ExecuteApply,
       ExecuteChunkDelivery, ExecuteRejectAuthenticatedJunk,
       CompleteProposalSignature, CompleteVoteSignature, FormPrepareQC,
       ApplyDecision, PublishControlItems,
       PublishControlAndEphemeralItems, PublishCertifiedRequests,
       RetireNodeCertifiedResponseAuthority,
       RememberedControl, RetainedClassItems, ControlClass,
       ProposalOutbox, VoteOutbox, QcOutbox,
       TimeoutViewOwnershipKernelProjection,
       TimeoutOwnershipRetainedItems,
       TimeoutOwnershipRetainedItemsIn,
       TimeoutOwnershipControlKinds,
       AsyncRecoveryControlVars, AsyncAuxVars, vars

THEOREM ExecuteCommandPreservesTimeoutViewOwnershipKernel ==
  \A command:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ DecisionFrontierUniquenessInvariant
    /\ DecisionTimeoutFrontierInvariant
    /\ TimeoutViewOwnershipKernelInvariant
    /\ ExecuteCommand(command)
    /\ UNCHANGED AsyncRecoveryControlVars
    => TimeoutViewOwnershipKernelInvariant'
PROOF
  <1>1. ASSUME NEW command,
              AsyncStrongTypeInvariant,
              AsyncProgressOwnershipInvariant,
              DecisionFrontierUniquenessInvariant,
              DecisionTimeoutFrontierInvariant,
              TimeoutViewOwnershipKernelInvariant,
              ExecuteCommand(command),
              UNCHANGED AsyncRecoveryControlVars
         PROVE TimeoutViewOwnershipKernelInvariant'
    <2>1. CASE ExecuteRegularCommand(command)
      BY <1>1, <2>1,
         ExecuteRegularCommandPreservesTimeoutViewOwnershipKernel
    <2>2. CASE ExecuteSignTimeout(command)
      BY <1>1, <2>2,
         ExecuteSignTimeoutPreservesTimeoutViewOwnershipKernel
    <2>3. CASE ExecutePersistInstall(command)
      BY <1>1, <2>3,
         ExecutePersistInstallPreservesTimeoutViewOwnershipKernel
    <2>4. CASE ExecutePersistDecision(command)
      BY <1>1, <2>4,
         ExecutePersistDecisionPreservesTimeoutViewOwnershipKernel
    <2>5. CASE ExecuteCoreDelivery(command)
      BY <1>1, <2>5,
         ExecuteCoreDeliveryPreservesTimeoutViewOwnershipKernel
    <2>6. CASE /\ ~ExecuteRegularCommand(command)
                 /\ ~ExecuteSignTimeout(command)
                 /\ ~ExecutePersistInstall(command)
                 /\ ~ExecutePersistDecision(command)
                 /\ ~ExecuteCoreDelivery(command)
      <3>1. UNCHANGED TimeoutViewOwnershipKernelProjection
        BY <1>1, <2>6,
           ExecuteOtherCommandLeavesTimeoutOwnershipProjection
      <3> QED BY <1>1, <3>1,
           TimeoutViewOwnershipKernelProjectionFrame
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4, <2>5, <2>6
         DEF ExecuteCommand
  <1> QED BY <1>1

THEOREM LocalTimeoutStepPreservesTimeoutViewOwnershipKernel ==
  \A node:
    /\ AsyncStrongTypeInvariant
    /\ TimeoutViewOwnershipKernelInvariant
    /\ (DirectTimeoutStep(node) \/ DeferredTimeoutStep(node))
    /\ UNCHANGED AsyncRecoveryControlVars
    => TimeoutViewOwnershipKernelInvariant'
BY IsaM("blast")
   DEF AsyncStrongTypeInvariant,
       StrongInductiveInvariant, Safety, TypeInvariant,
       ReducerProvenanceInvariant, PendingVoteWritesAuthorized,
       TimeoutViewOwnershipKernelInvariant,
       ResponsiveRetainedTimeoutOwnershipControlSound,
       ResponsiveRetainedTimeoutVoteControlSound,
       ResponsiveRetainedTcControlSound,
       ResponsiveRetainedDecisionControlSound,
       ResponsiveInstalledTcAuthorityInvariant,
       ResponsiveTimeoutVoteAuthorityInvariant,
       ResponsiveTimeoutQuorumAuthorityInvariant,
       ResponsiveDecisionCertificateAuthorityInvariant,
       RetainedViewCertificateAuthority,
       TimeoutVoteLifecycleAuthority,
       TimeoutVoteConcreteAuthority,
       RetainedTimeoutVoteAuthority,
       TimeoutReplayRecoveryAuthority,
       TimeoutReceiptQuorumInstallAuthority,
       DecisionCertificateRetainedAuthority,
       TimeoutVoteSemanticIdentity,
       TimeoutCertificateSemanticIdentity,
       DecisionSourceAt, NodeHasDecision,
       ResponsiveTimeoutReceiptQuorumAt, ReceivedTimeoutVoteAt,
       DirectTimeoutStep, DeferredTimeoutStep,
       BeginTimeoutEnabled, BeginTimeout, BeginTimeoutReady,
       TimeoutRequestFor, LocalTimeoutVoteFor,
       AppendCausalSuccessors, LeaveCausalQueues,
       TimeoutViewOwnershipKernelProjection,
       TimeoutOwnershipRetainedItems,
       TimeoutOwnershipRetainedItemsIn,
       TimeoutOwnershipControlKinds,
       AsyncRecoveryControlVars, vars,
       AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch

THEOREM RuntimeStepPreservesTimeoutViewOwnershipKernel ==
  \A node:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ DecisionFrontierUniquenessInvariant
    /\ DecisionTimeoutFrontierInvariant
    /\ TimeoutViewOwnershipKernelInvariant
    /\ RuntimeStep(node)
    /\ UNCHANGED AsyncRecoveryControlVars
    => TimeoutViewOwnershipKernelInvariant'
BY ExecuteCommandPreservesTimeoutViewOwnershipKernel,
   TimeoutFormingCommandCreatesExactInstallAuthority,
   LocalTimeoutStepPreservesTimeoutViewOwnershipKernel,
   TimeoutViewOwnershipKernelProjectionFrame,
   IsaT(1200)
   DEF RuntimeStep, FifoRuntimeStep, DeferredDrainStep,
       DeferredTagStep, DeferredTimeoutStep, DeferredRetransmitStep,
       DirectTimeoutStep, DirectRetransmitStep, IdleRuntimeStep,
       DeferCommand, DiscardCommand,
       RemoveNextNodeCommand, RemoveNextDeferredCommand,
       ClearDeferredHandoff, RetainDeferredHandoffs,
       AdvanceNextDeferredClass, InstallDeferredHandoff,
       AppendCausalSuccessors, LeaveCausalQueues,
       SendNodeRetransmissions, NoSendItem,
       TimeoutViewOwnershipKernelProjection,
       TimeoutOwnershipRetainedItems,
       TimeoutOwnershipRetainedItemsIn,
       TimeoutOwnershipControlKinds,
       AsyncRecoveryControlVars, vars

THEOREM ReplayRunNodeContinuationPreservesTimeoutViewOwnershipKernel ==
  \A node:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ DecisionFrontierUniquenessInvariant
    /\ DecisionTimeoutFrontierInvariant
    /\ TimeoutViewOwnershipKernelInvariant
    /\ ReplayRunNodeCandidateProducerContinuation(node)
    /\ UNCHANGED AsyncRecoveryControlVars
    => TimeoutViewOwnershipKernelInvariant'
PROOF
  <1>1. ASSUME NEW node,
                AsyncStrongTypeInvariant,
                AsyncProgressOwnershipInvariant,
                DecisionFrontierUniquenessInvariant,
                DecisionTimeoutFrontierInvariant,
                TimeoutViewOwnershipKernelInvariant,
                ReplayRunNodeCandidateProducerContinuation(node),
                UNCHANGED AsyncRecoveryControlVars
         PROVE TimeoutViewOwnershipKernelInvariant'
    <2>1. CASE
              AsyncCandidateProducerContinuationExactLocalReplayStep(node)
      BY <1>1, <2>1, TimeoutViewOwnershipKernelProjectionFrame, Isa
         DEF AsyncCandidateProducerContinuationExactLocalReplayStep,
             TimeoutViewOwnershipKernelProjection,
             TimeoutOwnershipRetainedItems,
             TimeoutOwnershipRetainedItemsIn,
             TimeoutOwnershipControlKinds,
             AsyncRecoveryControlVars, vars
    <2>2. CASE
              AsyncCandidateProducerContinuationReplayTargetOnlyTurn(node)
      BY <1>1, <2>2, TimeoutViewOwnershipKernelProjectionFrame, Isa
         DEF AsyncCandidateProducerContinuationReplayTargetOnlyTurn,
             TimeoutViewOwnershipKernelProjection,
             TimeoutOwnershipRetainedItems,
             TimeoutOwnershipRetainedItemsIn,
             TimeoutOwnershipControlKinds,
             AsyncRecoveryControlVars, vars
    <2>3. CASE
              AsyncCandidateProducerContinuationExactRuntimeReplayStep(node)
      <3>1. RuntimeStep(node)
        BY <2>3, Isa
           DEF AsyncCandidateProducerContinuationExactRuntimeReplayStep,
               RuntimeStep
      <3> QED BY <1>1, <3>1,
           RuntimeStepPreservesTimeoutViewOwnershipKernel
    <2> QED BY <1>1, <2>1, <2>2, <2>3
         DEF ReplayRunNodeCandidateProducerContinuation
  <1> QED BY <1>1

THEOREM RunNodeWorkPreservesTimeoutViewOwnershipKernel ==
  \A node:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ DecisionFrontierUniquenessInvariant
    /\ DecisionTimeoutFrontierInvariant
    /\ TimeoutViewOwnershipKernelInvariant
    /\ RunNodeWork(node)
    /\ UNCHANGED AsyncRecoveryControlVars
    => TimeoutViewOwnershipKernelInvariant'
BY RuntimeStepPreservesTimeoutViewOwnershipKernel,
   ReplayRunNodeContinuationPreservesTimeoutViewOwnershipKernel,
   TimeoutViewOwnershipKernelProjectionFrame,
   IsaT(600)
   DEF RunNodeWork, LocalAdmissionStep, IngressDrainStep,
       SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       SerializedLocalPrecedesServeIngressStep,
       SelectedLocalAdmissionAdvance,
       AsyncServeIngressTargetOnlyTurn,
       ResolveRunNodeCandidateProducerContinuation,
       AsyncSchedulerExceptCausalControlAndNodeService,
       AdmitProducerCompletion,
       AdmitCausalHead, UpdateLocalAdmissionMetadata,
       RecordBlockedCausalDebt, DrainFairIngressSelected,
       LeaveCausalQueues,
       TimeoutViewOwnershipKernelProjection,
       TimeoutOwnershipRetainedItems,
       TimeoutOwnershipRetainedItemsIn,
       TimeoutOwnershipControlKinds,
       AsyncRecoveryControlVars, vars

THEOREM AsyncRunnerStepPreservesTimeoutViewOwnershipKernel ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncProgressOwnershipInvariant
  /\ DecisionFrontierUniquenessInvariant
  /\ DecisionTimeoutFrontierInvariant
  /\ TimeoutViewOwnershipKernelInvariant
  /\ AsyncRunnerStep
  /\ UNCHANGED AsyncRecoveryControlVars
  => TimeoutViewOwnershipKernelInvariant'
BY RunNodeWorkPreservesTimeoutViewOwnershipKernel,
   TimeoutViewOwnershipKernelProjectionFrame,
   IsaT(600)
   DEF AsyncRunnerStep, RunNode, RunHistoricalRecoveryNode,
       RunHistoricalServer, DrainHistoricalIngressSelected,
       HistoricalIdleStep,
       TimeoutViewOwnershipKernelProjection,
       TimeoutOwnershipRetainedItems,
       TimeoutOwnershipRetainedItemsIn,
       TimeoutOwnershipControlKinds,
       AsyncRecoveryControlVars, vars

THEOREM PreGstNonresponsiveCrashPreservesTimeoutViewOwnershipKernel ==
  \A node:
    /\ AsyncStrongTypeInvariant
    /\ TimeoutViewOwnershipKernelInvariant
    /\ PreGstCrash(node)
    /\ UNCHANGED AsyncRecoveryControlVars
    => TimeoutViewOwnershipKernelInvariant'
BY IsaM("blast")
   DEF AsyncStrongTypeInvariant,
       StrongInductiveInvariant, Safety, TypeInvariant,
       TimeoutViewOwnershipKernelInvariant,
       ResponsiveRetainedTimeoutOwnershipControlSound,
       ResponsiveRetainedTimeoutVoteControlSound,
       ResponsiveRetainedTcControlSound,
       ResponsiveRetainedDecisionControlSound,
       ResponsiveInstalledTcAuthorityInvariant,
       ResponsiveTimeoutVoteAuthorityInvariant,
       ResponsiveTimeoutQuorumAuthorityInvariant,
       ResponsiveDecisionCertificateAuthorityInvariant,
       RetainedViewCertificateAuthority,
       TimeoutVoteLifecycleAuthority,
       TimeoutVoteConcreteAuthority,
       RetainedTimeoutVoteAuthority,
       TimeoutReplayRecoveryAuthority,
       TimeoutReceiptQuorumInstallAuthority,
       DecisionCertificateRetainedAuthority,
       TimeoutVoteSemanticIdentity,
       TimeoutCertificateSemanticIdentity,
       DecisionSourceAt, NodeHasDecision,
       ResponsiveTimeoutReceiptQuorumAt, ReceivedTimeoutVoteAt,
       PreGstCrash, Crash,
       TimeoutOwnershipRetainedItems,
       TimeoutOwnershipRetainedItemsIn,
       TimeoutOwnershipControlKinds,
       AsyncRecoveryControlVars, AsyncSchedulerVars, vars,
       AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch,
       RequestNodeSet

THEOREM AsyncSetGSTDischargesTimeoutReplayAuthority ==
  /\ TimeoutViewOwnershipKernelInvariant
  /\ AsyncSetGST
  => /\ TimeoutViewOwnershipKernelInvariant'
     /\ \A node \in AsyncCurrentResponsiveVoters,
          roundView \in Views, vote \in timeoutIntents:
          /\ TimeoutVoteSemanticIdentity(node, roundView, vote)
          /\ nodeView[node] = roundView
          /\ ~NodeHasDecision(node)
          => /\ TimeoutVoteConcreteAuthority(node, vote)
             /\ TimeoutVoteConcreteAuthority(node, vote)'
BY TimeoutViewOwnershipKernelProjectionFrame, IsaM("blast")
   DEF AsyncSetGST, SetGST,
       TimeoutViewOwnershipKernelInvariant,
       ResponsiveTimeoutVoteAuthorityInvariant,
       TimeoutVoteLifecycleAuthority,
       TimeoutVoteConcreteAuthority,
       RetainedTimeoutVoteAuthority,
       TimeoutReplayRecoveryAuthority,
       TimeoutViewOwnershipKernelProjection,
       TimeoutOwnershipRetainedItems,
       TimeoutOwnershipRetainedItemsIn,
       TimeoutOwnershipControlKinds,
       AsyncSchedulerVars, AsyncRecoveryVars,
       AsyncRecoveryControlVars, AsyncNonRunnerOuterFrame,
       AsyncNonCrashOuterFrame, AsyncCoreOuterFrame,
       AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch

THEOREM AsyncNonRunnerStepPreservesTimeoutViewOwnershipKernel ==
  /\ AsyncStrongTypeInvariant
  /\ TimeoutViewOwnershipKernelInvariant
  /\ AsyncNonRunnerStep
  /\ UNCHANGED AsyncRecoveryControlVars
  => TimeoutViewOwnershipKernelInvariant'
PROOF
  <1>1. ASSUME AsyncStrongTypeInvariant,
              TimeoutViewOwnershipKernelInvariant,
              AsyncNonRunnerStep,
              UNCHANGED AsyncRecoveryControlVars
         PROVE TimeoutViewOwnershipKernelInvariant'
    <2>1. CASE AsyncSetGST
      BY <1>1, <2>1,
         AsyncSetGSTDischargesTimeoutReplayAuthority
    <2>2. CASE ~AsyncSetGST
      BY <1>1, <2>2,
         PreGstNonresponsiveCrashPreservesTimeoutViewOwnershipKernel,
         TimeoutViewOwnershipKernelProjectionFrame,
         IsaT(1200)
         DEF AsyncNonRunnerStep, AsyncTick,
             OpenHistoricalRecovery,
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
             AsyncFaultStep, PreGstLosePacket,
             PreGstServeReceiverCloseRollback,
             PreGstPendingServeReceiverCloseRollback,
             PreGstMaterializedServeReceiverCloseRollback,
             InjectByzantineNoise, InjectUntrustedTransportCompletion,
             InjectAuthenticatedJunk, InjectByzantineCertifiedRequest,
             AsyncByzantineProposal, AsyncByzantineVote,
             AsyncByzantineTimeout, PublishEphemeralItems,
             PublishCommitCertificateRequests,
             TimeoutViewOwnershipKernelProjection,
             TimeoutOwnershipRetainedItems,
             TimeoutOwnershipRetainedItemsIn,
             TimeoutOwnershipControlKinds,
             AsyncRecoveryControlVars, AsyncSchedulerVars,
             AsyncNonClockVars, vars
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM PreGstResponsiveCrashPreservesTimeoutViewOwnershipKernel ==
  \A node:
    /\ AsyncStrongTypeInvariant
    /\ TimeoutViewOwnershipKernelInvariant
    /\ PreGstResponsiveCrash(node)
    => TimeoutViewOwnershipKernelInvariant'
BY IsaM("blast")
   DEF AsyncStrongTypeInvariant,
       StrongInductiveInvariant, Safety, TypeInvariant,
       TimeoutViewOwnershipKernelInvariant,
       ResponsiveRetainedTimeoutOwnershipControlSound,
       ResponsiveRetainedTimeoutVoteControlSound,
       ResponsiveRetainedTcControlSound,
       ResponsiveRetainedDecisionControlSound,
       ResponsiveInstalledTcAuthorityInvariant,
       ResponsiveTimeoutVoteAuthorityInvariant,
       ResponsiveTimeoutQuorumAuthorityInvariant,
       ResponsiveDecisionCertificateAuthorityInvariant,
       RetainedViewCertificateAuthority,
       TimeoutVoteLifecycleAuthority,
       TimeoutVoteConcreteAuthority,
       RetainedTimeoutVoteAuthority,
       TimeoutReplayRecoveryAuthority,
       TimeoutReceiptQuorumInstallAuthority,
       DecisionCertificateRetainedAuthority,
       TimeoutVoteSemanticIdentity,
       TimeoutCertificateSemanticIdentity,
       DecisionSourceAt, NodeHasDecision,
       ResponsiveTimeoutReceiptQuorumAt, ReceivedTimeoutVoteAt,
       PreGstResponsiveCrash, Crash,
       TimeoutOwnershipRetainedItems,
       TimeoutOwnershipRetainedItemsIn,
       TimeoutOwnershipControlKinds,
       AsyncRecoveryControlVars, AsyncSchedulerVars, vars,
       AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch,
       RequestNodeSet

THEOREM PreGstResponsiveRestartPreservesTimeoutViewOwnershipKernel ==
  /\ AsyncStrongTypeInvariant
  /\ TimeoutViewOwnershipKernelInvariant
  /\ PreGstResponsiveRestart
  => TimeoutViewOwnershipKernelInvariant'
BY IsaM("blast")
   DEF AsyncStrongTypeInvariant,
       StrongInductiveInvariant, Safety, TypeInvariant,
       TimeoutViewOwnershipKernelInvariant,
       ResponsiveRetainedTimeoutOwnershipControlSound,
       ResponsiveRetainedTimeoutVoteControlSound,
       ResponsiveRetainedTcControlSound,
       ResponsiveRetainedDecisionControlSound,
       ResponsiveInstalledTcAuthorityInvariant,
       ResponsiveTimeoutVoteAuthorityInvariant,
       ResponsiveTimeoutQuorumAuthorityInvariant,
       ResponsiveDecisionCertificateAuthorityInvariant,
       RetainedViewCertificateAuthority,
       TimeoutVoteLifecycleAuthority,
       TimeoutVoteConcreteAuthority,
       RetainedTimeoutVoteAuthority,
       TimeoutReplayRecoveryAuthority,
       TimeoutReceiptQuorumInstallAuthority,
       DecisionCertificateRetainedAuthority,
       TimeoutVoteSemanticIdentity,
       TimeoutCertificateSemanticIdentity,
       DecisionSourceAt, NodeHasDecision,
       ResponsiveTimeoutReceiptQuorumAt, ReceivedTimeoutVoteAt,
       PreGstResponsiveRestart, Restart,
       TimeoutOwnershipRetainedItems,
       TimeoutOwnershipRetainedItemsIn,
       TimeoutOwnershipControlKinds,
       AsyncRecoveryControlVars, AsyncSchedulerVars, vars,
       AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch

THEOREM RestartRetainedControlRestoresLastTcAuthority ==
  \A node \in ValidatorIds:
    lastInstalledTc[node] # NoTimeoutCertificate
      => TcOutbox(node, lastInstalledTc[node])
           \subseteq RestartRetainedControl(node)
BY IsaM("blast")
   DEF RestartRetainedControl,
       RestartLastTCControl, RestartLastInstalledTCs,
       RestartDecisionControl, RestartDecisionQCs,
       RestartHighestPrepareControl,
       RememberedControl, RetainedClassItems,
       ControlClass, ControlView,
       TcOutbox, QcOutbox

THEOREM RestartRetainedControlRestoresDecisionAuthority ==
  \A node \in ValidatorIds, qc \in QcRecordSet:
    /\ DecisionsUniqueByNodeContext
    /\ DecisionSourceAt(node, qc)
    => QcOutbox(node, qc) \subseteq RestartRetainedControl(node)
BY IsaM("blast")
   DEF DecisionsUniqueByNodeContext,
       DecisionSourceAt, RestartRetainedControl,
       RestartDecisionControl, RestartDecisionQCs,
       RestartHighestPrepareControl,
       RestartLastTCControl, RestartLastInstalledTCs,
       RememberedControl, RetainedClassItems,
       ControlClass, ControlView,
       TcOutbox, QcOutbox

THEOREM PreGstResponsiveReplayPreservesTimeoutViewOwnershipKernel ==
  /\ AsyncStrongTypeInvariant
  /\ DecisionFrontierUniquenessInvariant
  /\ TimeoutViewOwnershipKernelInvariant
  /\ PreGstResponsiveReplay
  => TimeoutViewOwnershipKernelInvariant'
BY RestartRetainedControlRestoresLastTcAuthority,
   RestartRetainedControlRestoresDecisionAuthority,
   IsaM("blast")
   DEF AsyncStrongTypeInvariant,
       StrongInductiveInvariant, Safety, TypeInvariant,
       ReducerProvenanceInvariant, HonestTimeoutUnique,
       AsyncSerializedBusyKernelInvariant, AsyncBusyReadinessInvariant,
       DecisionFrontierUniquenessInvariant,
       DecisionsUniqueByNodeContext,
       TimeoutViewOwnershipKernelInvariant,
       ResponsiveRetainedTimeoutOwnershipControlSound,
       ResponsiveRetainedTimeoutVoteControlSound,
       ResponsiveRetainedTcControlSound,
       ResponsiveRetainedDecisionControlSound,
       ResponsiveInstalledTcAuthorityInvariant,
       ResponsiveTimeoutVoteAuthorityInvariant,
       ResponsiveTimeoutQuorumAuthorityInvariant,
       ResponsiveDecisionCertificateAuthorityInvariant,
       RetainedViewCertificateAuthority,
       TimeoutVoteLifecycleAuthority,
       TimeoutVoteConcreteAuthority,
       RetainedTimeoutVoteAuthority,
       TimeoutReplayRecoveryAuthority,
       TimeoutReceiptQuorumInstallAuthority,
       DecisionCertificateRetainedAuthority,
       TimeoutVoteSemanticIdentity,
       TimeoutCertificateSemanticIdentity,
       DecisionSourceAt, NodeHasDecision,
       ResponsiveTimeoutReceiptQuorumAt, ReceivedTimeoutVoteAt,
       PreGstResponsiveReplay, RecoveryCoreReplay,
       ResetNodeSchedulerForRestart, RestartRetainedControl,
       RestartSignatureReplay, RestartTimeoutOrProposalReplay,
       RestartTimeoutReplay, RestartTimeoutIntents,
       RestartTimeoutIntent, RestartDecisions,
       RestartPrepareReplayIfActive,
       RestartLockedCommitReplayIfActive,
       FreshRestartCandidateSequence,
       AsyncCandidateRestartReplayTombstoned,
       ResumeTimeout, TimeoutSign,
       RestartRetainedActiveRequests,
       CertifiedResponseClaimForRequestsExceptRecipient,
       TimeoutOwnershipRetainedItems,
       TimeoutOwnershipRetainedItemsIn,
       TimeoutOwnershipControlKinds,
       AsyncRecoveryControlVars, AsyncSchedulerVars, vars,
       AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch,
       RequestNodeSet

THEOREM DriveResponsiveReplayPreservesTimeoutViewOwnershipKernel ==
  /\ AsyncStrongTypeInvariant
  /\ TimeoutViewOwnershipKernelInvariant
  /\ DriveResponsiveReplayHead
  => TimeoutViewOwnershipKernelInvariant'
BY IsaM("blast")
   DEF AsyncStrongTypeInvariant,
       StrongInductiveInvariant, Safety, TypeInvariant,
       TimeoutViewOwnershipKernelInvariant,
       ResponsiveRetainedTimeoutOwnershipControlSound,
       ResponsiveRetainedTimeoutVoteControlSound,
       ResponsiveRetainedTcControlSound,
       ResponsiveRetainedDecisionControlSound,
       ResponsiveInstalledTcAuthorityInvariant,
       ResponsiveTimeoutVoteAuthorityInvariant,
       ResponsiveTimeoutQuorumAuthorityInvariant,
       ResponsiveDecisionCertificateAuthorityInvariant,
       RetainedViewCertificateAuthority,
       TimeoutVoteLifecycleAuthority,
       TimeoutVoteConcreteAuthority,
       RetainedTimeoutVoteAuthority,
       TimeoutReplayRecoveryAuthority,
       TimeoutReceiptQuorumInstallAuthority,
       DecisionCertificateRetainedAuthority,
       TimeoutVoteSemanticIdentity,
       TimeoutCertificateSemanticIdentity,
       DecisionSourceAt, NodeHasDecision,
       ResponsiveTimeoutReceiptQuorumAt, ReceivedTimeoutVoteAt,
       DriveResponsiveReplayHead, RecoveryCoreReplay,
       ResumeProposal, ResumeVote, ResumeTimeout,
       TimeoutOwnershipRetainedItems,
       TimeoutOwnershipRetainedItemsIn,
       TimeoutOwnershipControlKinds,
       AsyncRecoveryControlVars, AsyncSchedulerVars, vars,
       AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch

THEOREM FinishResponsiveReplayPreservesTimeoutViewOwnershipKernel ==
  /\ TimeoutViewOwnershipKernelInvariant
  /\ FinishResponsiveReplay
  => TimeoutViewOwnershipKernelInvariant'
BY Isa
   DEF TimeoutViewOwnershipKernelInvariant,
       ResponsiveRetainedTimeoutOwnershipControlSound,
       ResponsiveRetainedTimeoutVoteControlSound,
       ResponsiveRetainedTcControlSound,
       ResponsiveRetainedDecisionControlSound,
       ResponsiveInstalledTcAuthorityInvariant,
       ResponsiveTimeoutVoteAuthorityInvariant,
       ResponsiveTimeoutQuorumAuthorityInvariant,
       ResponsiveDecisionCertificateAuthorityInvariant,
       RetainedViewCertificateAuthority,
       TimeoutVoteLifecycleAuthority,
       TimeoutVoteConcreteAuthority,
       RetainedTimeoutVoteAuthority,
       TimeoutReplayRecoveryAuthority,
       TimeoutReceiptQuorumInstallAuthority,
       DecisionCertificateRetainedAuthority,
       TimeoutVoteSemanticIdentity,
       TimeoutCertificateSemanticIdentity,
       DecisionSourceAt, NodeHasDecision,
       ResponsiveTimeoutReceiptQuorumAt, ReceivedTimeoutVoteAt,
       FinishResponsiveReplay,
       TimeoutOwnershipRetainedItems,
       TimeoutOwnershipRetainedItemsIn,
       TimeoutOwnershipControlKinds,
       AsyncRecoveryControlVars, AsyncSchedulerVars, vars,
       AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch

THEOREM RearmResponsiveRecoveryPreservesTimeoutViewOwnershipKernel ==
  /\ TimeoutViewOwnershipKernelInvariant
  /\ RearmResponsiveRecovery
  => TimeoutViewOwnershipKernelInvariant'
BY Isa
   DEF TimeoutViewOwnershipKernelInvariant,
       ResponsiveRetainedTimeoutOwnershipControlSound,
       ResponsiveRetainedTimeoutVoteControlSound,
       ResponsiveRetainedTcControlSound,
       ResponsiveRetainedDecisionControlSound,
       ResponsiveInstalledTcAuthorityInvariant,
       ResponsiveTimeoutVoteAuthorityInvariant,
       ResponsiveTimeoutQuorumAuthorityInvariant,
       ResponsiveDecisionCertificateAuthorityInvariant,
       RetainedViewCertificateAuthority,
       TimeoutVoteLifecycleAuthority,
       TimeoutVoteConcreteAuthority,
       RetainedTimeoutVoteAuthority,
       TimeoutReplayRecoveryAuthority,
       TimeoutReceiptQuorumInstallAuthority,
       DecisionCertificateRetainedAuthority,
       TimeoutVoteSemanticIdentity,
       TimeoutCertificateSemanticIdentity,
       DecisionSourceAt, NodeHasDecision,
       ResponsiveTimeoutReceiptQuorumAt, ReceivedTimeoutVoteAt,
       RearmResponsiveRecovery,
       TimeoutOwnershipRetainedItems,
       TimeoutOwnershipRetainedItemsIn,
       TimeoutOwnershipControlKinds,
       AsyncRecoveryControlVars, AsyncSchedulerVars, vars,
       AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch

THEOREM AsyncNonCrashStepPreservesTimeoutViewOwnershipKernel ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncProgressOwnershipInvariant
  /\ DecisionFrontierUniquenessInvariant
  /\ DecisionTimeoutFrontierInvariant
  /\ TimeoutViewOwnershipKernelInvariant
  /\ AsyncNonCrashStep
  => TimeoutViewOwnershipKernelInvariant'
PROOF
  <1>1. ASSUME AsyncStrongTypeInvariant,
              AsyncProgressOwnershipInvariant,
              DecisionFrontierUniquenessInvariant,
              DecisionTimeoutFrontierInvariant,
              TimeoutViewOwnershipKernelInvariant,
              AsyncNonCrashStep
         PROVE TimeoutViewOwnershipKernelInvariant'
    <2>1. CASE /\ (AsyncRunnerStep \/ AsyncNonRunnerStep)
                 /\ UNCHANGED <<up, AsyncRecoveryControlVars>>
      <3>1. UNCHANGED AsyncRecoveryControlVars
        BY <2>1
      <3>2. CASE AsyncRunnerStep
        BY <1>1, <2>1, <3>1, <3>2,
           AsyncRunnerStepPreservesTimeoutViewOwnershipKernel
      <3>3. CASE AsyncNonRunnerStep
        BY <1>1, <2>1, <3>1, <3>3,
           AsyncNonRunnerStepPreservesTimeoutViewOwnershipKernel
      <3> QED BY <2>1, <3>2, <3>3
    <2>2. CASE DriveResponsiveReplayHead
      BY <1>1, <2>2,
         DriveResponsiveReplayPreservesTimeoutViewOwnershipKernel
    <2>3. CASE FinishResponsiveReplay
      BY <1>1, <2>3,
         FinishResponsiveReplayPreservesTimeoutViewOwnershipKernel
    <2>4. CASE RearmResponsiveRecovery
      BY <1>1, <2>4,
         RearmResponsiveRecoveryPreservesTimeoutViewOwnershipKernel
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4
         DEF AsyncNonCrashStep
  <1> QED BY <1>1

THEOREM AsyncNextPreservesTimeoutViewOwnershipKernel ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncProgressOwnershipInvariant
  /\ DecisionFrontierUniquenessInvariant
  /\ DecisionTimeoutFrontierInvariant
  /\ TimeoutViewOwnershipKernelInvariant
  /\ AsyncNext
  => TimeoutViewOwnershipKernelInvariant'
PROOF
  <1>1. ASSUME AsyncStrongTypeInvariant,
              AsyncProgressOwnershipInvariant,
              DecisionFrontierUniquenessInvariant,
              DecisionTimeoutFrontierInvariant,
              TimeoutViewOwnershipKernelInvariant,
              AsyncNext
         PROVE TimeoutViewOwnershipKernelInvariant'
    <2>1. CASE AsyncNonCrashStep
      BY <1>1, <2>1,
         AsyncNonCrashStepPreservesTimeoutViewOwnershipKernel
    <2>2. CASE \E node \in ValidatorIds:
                  AsyncEnterIndexedServiceActivation(node)
                    \/ AsyncActivateServiceNode(node)
      <3>1. PICK node \in ValidatorIds:
               AsyncEnterIndexedServiceActivation(node)
                 \/ AsyncActivateServiceNode(node)
        BY <2>2
      <3>2. UNCHANGED TimeoutViewOwnershipKernelProjection
        BY <3>1, Isa
           DEF AsyncEnterIndexedServiceActivation,
               AsyncActivateServiceNode,
               AsyncServiceActivationFrameVars,
               AsyncSchedulerExceptServiceActivation,
               TimeoutViewOwnershipKernelProjection,
               TimeoutOwnershipRetainedItems,
               TimeoutOwnershipRetainedItemsIn,
               AsyncRecoveryControlVars, vars
      <3> QED BY <1>1, <3>2,
           TimeoutViewOwnershipKernelProjectionFrame
    <2>3. CASE \E node \in ValidatorIds: PreGstCrash(node)
      <3>1. ASSUME NEW node \in ValidatorIds, PreGstCrash(node)
             PROVE TimeoutViewOwnershipKernelInvariant'
        <4>1. UNCHANGED AsyncRecoveryControlVars
          BY <3>1 DEF PreGstCrash, AsyncSchedulerVars
        <4> QED BY <1>1, <3>1, <4>1,
             PreGstNonresponsiveCrashPreservesTimeoutViewOwnershipKernel
      <3> QED BY <2>3, <3>1
    <2>4. CASE \E node \in ValidatorIds: PreGstResponsiveCrash(node)
      BY <1>1, <2>4,
         PreGstResponsiveCrashPreservesTimeoutViewOwnershipKernel
    <2>5. CASE PreGstResponsiveRestart
      BY <1>1, <2>5,
         PreGstResponsiveRestartPreservesTimeoutViewOwnershipKernel
    <2>6. CASE PreGstResponsiveReplay
      BY <1>1, <2>6,
         PreGstResponsiveReplayPreservesTimeoutViewOwnershipKernel
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4, <2>5, <2>6
         DEF AsyncNext
  <1> QED BY <1>1

THEOREM AsyncAllVarsStutterPreservesTimeoutViewOwnershipKernel ==
  /\ TimeoutViewOwnershipKernelInvariant
  /\ UNCHANGED AsyncAllVars
  => TimeoutViewOwnershipKernelInvariant'
BY TimeoutViewOwnershipKernelProjectionFrame, Isa
   DEF TimeoutViewOwnershipKernelProjection,
       TimeoutOwnershipRetainedItems,
       TimeoutOwnershipRetainedItemsIn,
       TimeoutOwnershipControlKinds,
       AsyncAllVars, AsyncSchedulerVars, AsyncRecoveryVars,
       AsyncRecoveryControlVars, vars

THEOREM AsyncBracketPreservesTimeoutViewOwnershipKernel ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncProgressOwnershipInvariant
  /\ DecisionFrontierUniquenessInvariant
  /\ DecisionTimeoutFrontierInvariant
  /\ TimeoutViewOwnershipKernelInvariant
  /\ [AsyncNext]_AsyncAllVars
  => TimeoutViewOwnershipKernelInvariant'
BY AsyncNextPreservesTimeoutViewOwnershipKernel,
   AsyncAllVarsStutterPreservesTimeoutViewOwnershipKernel, Isa

THEOREM TimeoutViewOwnershipKernelInvariantFromAsyncSpec ==
  \A initialContext:
    AsyncSpecAt(initialContext)
      => []TimeoutViewOwnershipKernelInvariant
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE AsyncSpecAt(initialContext)
                 => []TimeoutViewOwnershipKernelInvariant
    <2> DEFINE Inductive ==
           /\ AsyncStrongTypeInvariant
           /\ AsyncProgressOwnershipInvariant
           /\ DecisionFrontierUniquenessInvariant
           /\ DecisionTimeoutFrontierInvariant
           /\ TimeoutViewOwnershipKernelInvariant
    <2>1. AsyncInitAt(initialContext) => Inductive
      BY AsyncInitEstablishesStrongTypeInvariant,
         AsyncInitEstablishesProgressOwnership,
         AsyncInitEstablishesDecisionFrontierUniqueness,
         AsyncInitEstablishesDecisionTimeoutFrontier,
         AsyncInitEstablishesTimeoutViewOwnershipKernel
         DEF Inductive
    <2>2. Inductive /\ [AsyncNext]_AsyncAllVars => Inductive'
      BY AsyncBracketNextPreservesStrongTypeInvariant,
         AsyncBracketNextPreservesProgressOwnership,
         AsyncBracketPreservesStrongDecisionFrontier,
         AsyncBracketPreservesDecisionTimeoutFrontier,
         AsyncBracketPreservesTimeoutViewOwnershipKernel
         DEF Inductive
    <2>3. AsyncSpecAt(initialContext) => []Inductive
      BY <2>1, <2>2, PTL DEF AsyncSpecAt
    <2>4. Inductive => TimeoutViewOwnershipKernelInvariant
      BY DEF Inductive
    <2> QED BY <2>3, <2>4, PTL
  <1> QED BY <1>1

THEOREM TimeoutViewOwnershipInvariantFromAsyncSpec ==
  \A initialContext:
    AsyncSpecAt(initialContext) => []TimeoutViewOwnershipInvariant
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE AsyncSpecAt(initialContext)
                 => []TimeoutViewOwnershipInvariant
    <2>1. AsyncSpecAt(initialContext)
             => []TimeoutViewOwnershipKernelInvariant
      BY TimeoutViewOwnershipKernelInvariantFromAsyncSpec
    <2>2. AsyncSpecAt(initialContext) => []AsyncStrongTypeInvariant
      BY AsyncSpecAlwaysStrongTypeInvariant
    <2>3. /\ AsyncStrongTypeInvariant
           /\ TimeoutViewOwnershipKernelInvariant
          => TimeoutViewOwnershipInvariant
      BY TimeoutViewOwnershipKernelProjectsPublicInvariant
    <2> QED BY <2>1, <2>2, <2>3, PTL
  <1> QED BY <1>1

THEOREM TimeoutMissingRanksAreNatural ==
  AsyncTypeInvariant
    => /\ \A roundView:
             TimeoutMissingCatchupRank(roundView) \in Nat
       /\ \A target, receiptView:
             TimeoutMissingReceiptRank(target, receiptView) \in Nat
PROOF
  <1>1. ASSUME AsyncTypeInvariant
         PROVE /\ \A roundView:
                       TimeoutMissingCatchupRank(roundView) \in Nat
                /\ \A target, receiptView:
                       TimeoutMissingReceiptRank(target, receiptView) \in Nat
    <2>1. IsFiniteSet(AsyncCurrentResponsiveVoters)
      BY <1>1, RuntimeValidatorIdsAreFinite, FS_Subset, Isa
         DEF AsyncTypeInvariant, AsyncCurrentResponsiveVoters,
             CurrentVoters, CurrentEpoch
    <2>2. \A roundView:
             IsFiniteSet(TimeoutMissingCatchupVoters(roundView))
      BY <2>1, FS_Subset DEF TimeoutMissingCatchupVoters
    <2>3. \A target, receiptView:
             IsFiniteSet(
               TimeoutMissingReceiptVoters(target, receiptView))
      BY <2>1, FS_Subset DEF TimeoutMissingReceiptVoters
    <2> QED BY <2>2, <2>3, FS_CardinalityType
         DEF TimeoutMissingCatchupRank, TimeoutMissingReceiptRank
  <1> QED BY <1>1

THEOREM ResponsiveAuthoritySuppliesEveryTcFrontier ==
  \A source \in AsyncCurrentResponsiveVoters,
     recipient \in CurrentVoters:
    \A minimumView:
      ResponsiveViewCertificateAuthority(source, minimumView)
        => TcFrontier(recipient, minimumView)
BY Isa
   DEF ResponsiveViewCertificateAuthority, TcFrontier,
       TimeoutCertificateDelivery, TimeoutCertificateItem,
       TimeoutCertificateSemanticIdentity

THEOREM ResponsiveReceiptQuorumBuildsValidTc ==
  \A target \in ValidatorIds, roundView \in Views:
    /\ StrongInductiveInvariant
    /\ ReceivedTimeoutVotePoolInvariant
    /\ ResponsiveTimeoutReceiptQuorumAt(target, roundView)
    => TCValid(TC(context, roundView,
                   TimeoutVotesAt(target, roundView)))
BY ResponsiveReceiptsMakeDualQuorum,
   TimeoutPoolMakesVotesDisjoint,
   SameViewCertificateUniqueness,
   SMTT(60)
   DEF StrongInductiveInvariant, Safety, TypeInvariant,
       ReducerProvenanceInvariant, HonestTimeoutTransportBacked,
       ReceivedTimeoutVotePoolInvariant,
       ResponsiveTimeoutReceiptQuorumAt, ReceivedTimeoutVoteAt,
       TimeoutVotesAt, TimeoutSignerSet, TimeoutHighsConflictFree,
       AuthenticatedHighRef, HighRefValid, TC, TCValid,
       TimeoutVoteAt, CurrentVoters, CurrentEpoch

=============================================================================
