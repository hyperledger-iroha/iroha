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
  /\ item.source \in AsyncIngressSources
  /\ item.envelope.recipient \in ValidatorIds
  /\ item \in SequenceSet(
       IngressLane(item.envelope.recipient, item.source))

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
