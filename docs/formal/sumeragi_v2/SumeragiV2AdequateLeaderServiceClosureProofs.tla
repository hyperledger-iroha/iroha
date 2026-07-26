---- MODULE SumeragiV2AdequateLeaderServiceClosureProofs ----
EXTENDS SumeragiV2RotatingLeaderProgressProofs

(***************************************************************************
Exact adequate-leader service decomposition.

`AsyncLiveNormalProposalPrepareRanksProgress` and
`StarvationFreedomObligation` prove that an admitted protected candidate
eventually leaves scheduler ownership.  They do not say why it leaves.
The distinction matters:

  1. a successful reducer execution creates its declared causal successor or
     an exact durable/wire milestone;
  2. a TC installation can make the immutable consumer view/generation stale,
     which is itself pacemaker progress; and
  3. an idle, same-consumer `DiscardCommand` can remove a disabled candidate.

This module names those cases separately and projects the proved scheduler
starvation theorem onto exact view-change, proposal, Prepare, Commit, and
Decision ranks.  It also names the two non-candidate corridors which the
scheduler rank cannot cover: packet-to-ingress-to-candidate admission and
same-round timeout-quorum/view rotation.

No release aggregate is asserted.  In particular, this module does not prove
`AdequateLeaderServiceKernelProperty` for the live specification.  The exact
temporal sub-kernels at the bottom are explicit proof boundaries strictly
below that release target.  The recipient-local certified-response capacity
arm is discharged here; the remaining transport/runner/timeout property and
the semantic-composition property stay explicit and conditional.
***************************************************************************)

(***************************************************************************
Semantic ranks.

The first component is the protocol phase.  A larger component is earlier:
view change precedes Proposal, which precedes Prepare, Commit, and Decision.
The second component orders the concrete command/evidence stages inside one
phase.  `SignVote` is classified from the exact pending sign request, because
the command kind alone does not carry the Prepare/Commit phase.
***************************************************************************)

ViewChangeSemanticRank(step) == <<5, step>>
ProposalSemanticRank(step) == <<4, step>>
PrepareSemanticRank(step) == <<3, step>>
CommitSemanticRank(step) == <<2, step>>
DecisionSemanticRank(step) == <<1, step>>
TerminalSemanticRank == <<0, 0>>

SemanticRankLess(left, right) ==
  \/ left[1] < right[1]
  \/ /\ left[1] = right[1]
        /\ left[2] < right[2]

MatchingVoteSignRequest(candidate, phase) ==
  \E request \in signVotes:
    /\ request.node = candidate.node
    /\ request.vote.context = candidate.consumerContext
    /\ request.vote.view = candidate.view
    /\ request.vote.subject = candidate.subject
    /\ request.vote.phase = phase

ExactLeaderViewChangeRank(candidate, rank) ==
  \/ /\ candidate.kind = "PersistTimeout"
        /\ rank = ViewChangeSemanticRank(7)
  \/ /\ candidate.kind = "SignTimeout"
        /\ rank = ViewChangeSemanticRank(6)
  \/ /\ candidate.kind = "DeliverTimeout"
        /\ candidate.item.kind = "TimeoutVote"
        /\ rank = ViewChangeSemanticRank(5)
  \/ /\ candidate.kind = "FormTC"
        /\ rank = ViewChangeSemanticRank(4)
  \/ /\ candidate.kind = "DeliverTC"
        /\ candidate.item.kind = "TimeoutCertificate"
        /\ rank = ViewChangeSemanticRank(4)
  \/ /\ candidate.kind = "BeginInstallTC"
        /\ rank = ViewChangeSemanticRank(3)
  \/ /\ candidate.kind = "PersistInstallTC"
        /\ rank = ViewChangeSemanticRank(2)

ExactLeaderProposalRank(candidate, rank) ==
  \/ /\ candidate.kind = "AssembleBody"
        /\ candidate.node = Leader(candidate.consumerContext,
                                    candidate.view)
        /\ rank = ProposalSemanticRank(9)
  \/ /\ candidate.kind = "BeginProposal"
        /\ rank = ProposalSemanticRank(8)
  \/ /\ candidate.kind = "PersistProposal"
        /\ rank = ProposalSemanticRank(7)
  \/ /\ candidate.kind = "SignProposal"
        /\ rank = ProposalSemanticRank(6)
  \/ /\ candidate.kind \in {"DeliverProposal", "DeliverChunk"}
        /\ rank = ProposalSemanticRank(5)
  \/ /\ candidate.kind
          \in {"FetchBody", "RebindRetainedBody", "FetchCertifiedBody"}
        /\ rank = ProposalSemanticRank(4)
  \/ /\ candidate.kind = "StoreBody"
        /\ rank = ProposalSemanticRank(3)
  \/ /\ candidate.kind = "ValidateBody"
        /\ rank = ProposalSemanticRank(2)

ExactLeaderPrepareStaticRank(candidate, rank) ==
  \/ /\ candidate.kind = "BeginPrepare"
        /\ rank = PrepareSemanticRank(9)
  \/ /\ candidate.kind = "PersistPrepare"
        /\ rank = PrepareSemanticRank(8)
  \/ /\ candidate.kind = "DeliverVote"
        /\ candidate.item.kind = "PrepareVote"
        /\ rank = PrepareSemanticRank(6)
  \/ /\ candidate.kind = "FormPrepareQC"
        /\ rank = PrepareSemanticRank(5)
  \/ /\ candidate.kind = "DeliverQC"
        /\ candidate.item.kind = "PrepareQC"
        /\ rank = PrepareSemanticRank(4)
  \/ /\ candidate.kind = "BeginObservePrepare"
        /\ rank = PrepareSemanticRank(3)
  \/ /\ candidate.kind = "PersistObservePrepare"
        /\ rank = PrepareSemanticRank(2)

ExactLeaderPrepareSignRank(candidate, rank) ==
  /\ candidate.kind = "SignVote"
  /\ MatchingVoteSignRequest(candidate, "Prepare")
  /\ rank = PrepareSemanticRank(7)

ExactLeaderPrepareRank(candidate, rank) ==
  \/ ExactLeaderPrepareStaticRank(candidate, rank)
  \/ ExactLeaderPrepareSignRank(candidate, rank)

ExactLeaderCommitStaticRank(candidate, rank) ==
  \/ /\ candidate.kind = "BeginLockCommit"
        /\ rank = CommitSemanticRank(9)
  \/ /\ candidate.kind = "PersistLockCommit"
        /\ rank = CommitSemanticRank(8)
  \/ /\ candidate.kind = "DeliverVote"
        /\ candidate.item.kind = "CommitVote"
        /\ rank = CommitSemanticRank(6)
  \/ /\ candidate.kind = "FormCommitQC"
        /\ rank = CommitSemanticRank(5)
  \/ /\ candidate.kind = "DeliverQC"
        /\ candidate.item.kind = "CommitQC"
        /\ rank = CommitSemanticRank(4)

ExactLeaderCommitSignRank(candidate, rank) ==
  /\ candidate.kind = "SignVote"
  /\ MatchingVoteSignRequest(candidate, "Commit")
  /\ rank = CommitSemanticRank(7)

ExactLeaderCommitRank(candidate, rank) ==
  \/ ExactLeaderCommitStaticRank(candidate, rank)
  \/ ExactLeaderCommitSignRank(candidate, rank)

ExactLeaderDecisionRank(candidate, rank) ==
  \/ /\ candidate.kind = "BeginDecision"
        /\ rank = DecisionSemanticRank(3)
  \/ /\ candidate.kind = "PersistDecision"
        /\ rank = DecisionSemanticRank(2)

ExactLeaderPhaseRank(candidate, rank) ==
  \/ ExactLeaderViewChangeRank(candidate, rank)
  \/ ExactLeaderProposalRank(candidate, rank)
  \/ ExactLeaderPrepareRank(candidate, rank)
  \/ ExactLeaderCommitRank(candidate, rank)
  \/ ExactLeaderDecisionRank(candidate, rank)

ExactLeaderCandidateRank(candidate, rank) ==
  /\ ResponsiveProtectedCandidateOwned(candidate)
  /\ CandidateConsumerCurrent(candidate)
  /\ ExactLeaderPhaseRank(candidate, rank)

ExactLeaderServiceCandidate(candidate) ==
  \E rank \in (1..5) \X Nat:
    ExactLeaderCandidateRank(candidate, rank)

THEOREM ExactLeaderCandidateRankIsSemanticRank ==
  \A candidate, rank:
    ExactLeaderCandidateRank(candidate, rank)
      => /\ candidate \in AsyncCandidateSet
         /\ rank \in (1..5) \X Nat
         /\ ResponsiveProtectedCandidateOwned(candidate)
         /\ CandidateConsumerCurrent(candidate)
BY Isa
   DEF ExactLeaderCandidateRank,
       ViewChangeSemanticRank, ProposalSemanticRank,
       PrepareSemanticRank, CommitSemanticRank, DecisionSemanticRank,
       ResponsiveProtectedCandidateOwned, ProtectedCandidateOwned,
       ProtectedServiceCandidate

THEOREM SemanticRankOrderingIsLexicographic ==
  \A left, right \in (0..5) \X Nat:
    SemanticRankLess(left, right)
      <=> \/ left[1] < right[1]
          \/ /\ left[1] = right[1]
                /\ left[2] < right[2]
BY DEF SemanticRankLess

(***************************************************************************
Projection of proved physical scheduler starvation.

This is the part of adequate-leader service which is already closed.  It is
deliberately stated for every exact semantic rank, while the imported theorem
continues to use the independent physical carrier rank in stages 2..6.
***************************************************************************)

ExactLeaderRankedCandidateExitProperty(specification) ==
  specification
    => \A candidate \in AsyncCandidateSet,
          semanticRank \in (1..5) \X Nat:
         (gst /\ ExactLeaderCandidateRank(candidate, semanticRank))
           ~> ~ResponsiveProtectedCandidateOwned(candidate)

THEOREM AsyncLiveExactLeaderRankedCandidatesExit ==
  \A initialContext:
    ExactLeaderRankedCandidateExitProperty(
      AsyncLiveSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE ExactLeaderRankedCandidateExitProperty(
                 AsyncLiveSpecAt(initialContext))
    <2>1. StarvationFreedomProperty(
             AsyncLiveSpecAt(initialContext))
      BY StarvationFreedomObligation
    <2>2. ASSUME AsyncLiveSpecAt(initialContext)
           PROVE \A candidate \in AsyncCandidateSet,
                     semanticRank \in (1..5) \X Nat:
              (gst
                /\ ExactLeaderCandidateRank(candidate, semanticRank))
                ~> ~ResponsiveProtectedCandidateOwned(candidate)
      <3>1. ASSUME NEW candidate \in AsyncCandidateSet,
                    NEW semanticRank \in (1..5) \X Nat
             PROVE (gst
                      /\ ExactLeaderCandidateRank(
                           candidate, semanticRank))
                     ~> ~ResponsiveProtectedCandidateOwned(candidate)
        <4>1. (gst /\ ResponsiveProtectedCandidateOwned(candidate))
                 ~> ~ResponsiveProtectedCandidateOwned(candidate)
          BY <2>1, <2>2 DEF StarvationFreedomProperty
        <4>2. ExactLeaderCandidateRank(candidate, semanticRank)
                 => ResponsiveProtectedCandidateOwned(candidate)
          BY ExactLeaderCandidateRankIsSemanticRank
        <4> QED BY <4>1, <4>2, PTL
      <3> QED BY <3>1
    <2> QED BY <2>2
         DEF ExactLeaderRankedCandidateExitProperty
  <1> QED BY <1>1

(***************************************************************************
Exact reducer milestones.

Declared causal successors cover the local WAL/body/reducer chain.  Four
successful commands instead publish an external wire frontier, and
PersistDecision/PersistInstallTC create terminal or pacemaker evidence.
Certified Fetch may open an exact signed request without a local successor.
***************************************************************************)

DeclaredSuccessorsOwned(candidate) ==
  /\ CommandSuccessors(candidate) # <<>>
  /\ \A successor \in SequenceSet(CommandSuccessors(candidate)):
       CandidateScheduled(successor)

SentProposalAt(source, recipient, roundView, subject) ==
  \E item \in asyncSentItems:
    /\ item.kind = "Proposal"
    /\ item.source = source
    /\ item.envelope.recipient = recipient
    /\ item.envelope.proposal.view = roundView
    /\ item.envelope.proposal.subject = subject

SentVoteAt(source, recipient, roundView, phase, subject) ==
  \E item \in asyncSentItems:
    /\ item.kind =
         IF phase = "Prepare" THEN "PrepareVote" ELSE "CommitVote"
    /\ item.source = source
    /\ item.envelope.recipient = recipient
    /\ item.envelope.vote.view = roundView
    /\ item.envelope.vote.phase = phase
    /\ item.envelope.vote.subject = subject
    /\ item.envelope.vote.signer = source

SentQcAt(source, recipient, roundView, phase, subject) ==
  \E item \in asyncSentItems:
    /\ item.kind = IF phase = "Prepare" THEN "PrepareQC" ELSE "CommitQC"
    /\ item.source = source
    /\ item.envelope.recipient = recipient
    /\ item.envelope.qc.view = roundView
    /\ item.envelope.qc.phase = phase
    /\ item.envelope.qc.subject = subject

SentTimeoutAt(source, recipient, roundView) ==
  \E item \in asyncSentItems:
    /\ item.kind = "TimeoutVote"
    /\ item.source = source
    /\ item.envelope.recipient = recipient
    /\ item.envelope.vote.view = roundView
    /\ item.envelope.vote.signer = source

ProposalPublished(candidate) ==
  \A recipient \in CurrentVoters:
    SentProposalAt(candidate.node, recipient,
                   candidate.view, candidate.subject)

VotePublished(candidate, phase) ==
  /\ \E vote \in IF phase = "Prepare"
                 THEN prepareIntents ELSE commitIntents:
       /\ vote.context = candidate.consumerContext
       /\ vote.view = candidate.view
       /\ vote.phase = phase
       /\ vote.subject = candidate.subject
       /\ vote.signer = candidate.node
       /\ VoteAt(candidate.node, vote) \in receivedVotes
  /\ \A recipient \in CurrentVoters \ {candidate.node}:
       SentVoteAt(candidate.node, recipient, candidate.view,
                  phase, candidate.subject)

QcPublished(candidate, phase) ==
  \E qc \in IF phase = "Prepare" THEN prepareQCs ELSE commitQCs:
    /\ qc.context = candidate.consumerContext
    /\ qc.view = candidate.view
    /\ qc.phase = phase
    /\ qc.subject = candidate.subject
    /\ \A recipient \in CurrentVoters:
         SentQcAt(candidate.node, recipient, candidate.view,
                  phase, candidate.subject)

TimeoutPublished(candidate) ==
  \A recipient \in CurrentVoters:
    SentTimeoutAt(candidate.node, recipient, candidate.view)

CertifiedRequestOpened(candidate) ==
  \E request \in asyncActiveRequests:
    /\ request.kind = "CertifiedRequest"
    /\ request.source = candidate.node
    /\ request.envelope.height = candidate.height
    /\ request.envelope.view = candidate.view
    /\ request.envelope.subject = candidate.subject

ExactLeaderCandidatePostMilestone(candidate, rank) ==
  \/ DeclaredSuccessorsOwned(candidate)
  \/ /\ candidate.kind = "SignProposal"
        /\ ProposalPublished(candidate)
  \/ /\ candidate.kind = "SignVote"
        /\ rank[1] = 3
        /\ VotePublished(candidate, "Prepare")
  \/ /\ candidate.kind = "SignVote"
        /\ rank[1] = 2
        /\ VotePublished(candidate, "Commit")
  \/ /\ candidate.kind = "FormPrepareQC"
        /\ QcPublished(candidate, "Prepare")
  \/ /\ candidate.kind = "SignTimeout"
        /\ TimeoutPublished(candidate)
  \/ /\ candidate.kind \in {"FetchBody", "FetchCertifiedBody"}
        /\ CertifiedRequestOpened(candidate)
  \/ /\ candidate.kind = "PersistInstallTC"
        /\ nodeView[candidate.node] > candidate.consumerView
        /\ generation[candidate.node] >
             candidate.consumerGeneration
  \/ /\ candidate.kind = "PersistDecision"
        /\ NodeHasDecision(candidate.node)

THEOREM FifoSuccessfulLeaderExecutionSchedulesDeclaredSuccessors ==
  \A node \in ValidatorIds:
    LET candidate == NextNodeCommand(node)
    IN /\ AsyncStrongTypeInvariant
       /\ AsyncProgressOwnershipInvariant
       /\ ExactLeaderServiceCandidate(candidate)
       /\ NodeQueueNonempty(node)
       /\ CommandDispatchable(candidate)
       /\ FifoRuntimeStep(node)
       => (CommandSuccessors(candidate) = <<>>
             \/ DeclaredSuccessorsOwned(candidate)')
BY FifoSuccessfulExecutionSchedulesEverySuccessor
   DEF DeclaredSuccessorsOwned,
       CommandSuccessorsScheduledAfter

THEOREM DeferredSuccessfulLeaderExecutionSchedulesDeclaredSuccessors ==
  \A node \in ValidatorIds:
    LET candidate == NextDeferredCommand(node)
    IN /\ AsyncStrongTypeInvariant
       /\ AsyncProgressOwnershipInvariant
       /\ ExactLeaderServiceCandidate(candidate)
       /\ DeferredWorkServiceable(node)
       /\ CommandDispatchable(candidate)
       /\ DeferredDrainStep(node)
       => (CommandSuccessors(candidate) = <<>>
             \/ DeclaredSuccessorsOwned(candidate)')
BY DeferredSuccessfulExecutionSchedulesEverySuccessor
   DEF DeclaredSuccessorsOwned,
       CommandSuccessorsScheduledAfter

THEOREM ExecuteSignProposalCreatesExactWireMilestone ==
  \A candidate:
    ExecuteSignProposal(candidate) => ProposalPublished(candidate)'
BY Isa
   DEF ExecuteSignProposal, CommandMatches,
       CompleteProposalSignature, PublishControlAndEphemeralItems,
       ProposalOutbox, ProposalPublished, SentProposalAt,
       AsyncNetworkItem, ProposalEnvelope

THEOREM ExecuteSignVoteCreatesPhaseExactWireMilestone ==
  \A candidate:
    ExecuteSignVote(candidate)
      => \/ VotePublished(candidate, "Prepare")'
         \/ VotePublished(candidate, "Commit")'
BY Isa
   DEF ExecuteSignVote, CommandMatches, CompleteVoteSignature,
       PublishControlItems, VoteOutbox, VotePublished, SentVoteAt,
       AsyncNetworkItem, VoteEnvelope

THEOREM ExecuteFormPrepareQcCreatesExactWireMilestone ==
  \A candidate:
    ExecuteFormPrepareQC(candidate)
      => QcPublished(candidate, "Prepare")'
BY Isa
   DEF ExecuteFormPrepareQC, FormPrepareQC, PublishControlItems,
       QcOutbox, QcPublished, SentQcAt,
       AsyncNetworkItem, QcEnvelope

THEOREM ExecuteSignTimeoutCreatesExactWireMilestone ==
  \A candidate:
    ExecuteSignTimeout(candidate) => TimeoutPublished(candidate)'
BY Isa
   DEF ExecuteSignTimeout, CommandMatches, CompleteTimeoutSignature,
       PublishControlItems, TimeoutOutbox, TimeoutPublished,
       SentTimeoutAt, AsyncNetworkItem, TimeoutEnvelope

THEOREM ExecutePersistInstallCreatesExactPacemakerMilestone ==
  \A candidate:
    /\ TypeInvariant
    /\ CandidateConsumerCurrent(candidate)
    /\ ExecutePersistInstall(candidate)
      => /\ nodeView'[candidate.node] > candidate.consumerView
         /\ generation'[candidate.node] >
              candidate.consumerGeneration
BY ExecutePersistInstallAdvancesCertifiedView,
   InstallAdvancesDeliveryGeneration, Isa
   DEF ExecutePersistInstall, CandidateConsumerCurrent,
       TimeoutViewGoal

THEOREM ExecutePersistDecisionCreatesExactDecisionMilestone ==
  \A candidate:
    ExecutePersistDecision(candidate) => NodeHasDecision(candidate.node)'
BY Isa
   DEF ExecutePersistDecision, CommandMatches,
       PersistDecision, NodeHasDecision

(***************************************************************************
Exit trichotomy.

The stale-consumer case is not counted as a scheduler defect.  Core view and
generation counters are monotone, so losing the frozen consumer identity
advances at least one pacemaker coordinate.  The remaining same-consumer
discard is covered only by an already existing exact protocol witness or a
different same-identity owner.  Anything else is the precise local residual.
***************************************************************************)

SameIdentityLeaderOwner(candidate) ==
  \E other \in AsyncCandidateSet,
     otherRank \in (1..5) \X Nat:
    /\ other # candidate
    /\ other.node = candidate.node
    /\ other.height = candidate.height
    /\ other.view = candidate.view
    /\ other.subject = candidate.subject
    /\ ExactLeaderCandidateRank(other, otherRank)

(***************************************************************************
The scheduler-origin invariant cannot use an arbitrary alternate owner as its
base case.  Two disabled owners would otherwise justify one another and an
idle discard of either would strand the survivor.  The readiness-only anchor
therefore names an independently dispatchable exact owner.  The unanchored
predicate above remains the semantic statement that an exact owner survived
the removal of its peer.
***************************************************************************)
DispatchableSameIdentityLeaderOwner(candidate) ==
  \E other \in AsyncCandidateSet,
     otherRank \in (1..5) \X Nat:
    /\ other # candidate
    /\ other.node = candidate.node
    /\ other.height = candidate.height
    /\ other.view = candidate.view
    /\ other.subject = candidate.subject
    /\ ExactLeaderCandidateRank(other, otherRank)
    /\ CommandDispatchable(other)

ProposalEvidenceAt(candidate) ==
  \/ BodyHeldBy(durableBodies, candidate.node,
                candidate.consumerContext,
                candidate.view, candidate.subject)
  \/ \E proposal \in proposalIntents:
       /\ proposal.context = candidate.consumerContext
       /\ proposal.view = candidate.view
       /\ proposal.subject = candidate.subject
  \/ \E seen \in seenProposals:
       /\ seen.node = candidate.node
       /\ seen.proposal.context = candidate.consumerContext
       /\ seen.proposal.view = candidate.view
       /\ seen.proposal.subject = candidate.subject

PrepareEvidenceAt(candidate) ==
  \/ \E vote \in prepareIntents:
       /\ vote.context = candidate.consumerContext
       /\ vote.view = candidate.view
       /\ vote.subject = candidate.subject
  \/ \E received \in receivedVotes:
       /\ received.node = candidate.node
       /\ received.vote.context = candidate.consumerContext
       /\ received.vote.view = candidate.view
       /\ received.vote.phase = "Prepare"
       /\ received.vote.subject = candidate.subject
  \/ \E qc \in prepareQCs:
       /\ qc.context = candidate.consumerContext
       /\ qc.view = candidate.view
       /\ qc.subject = candidate.subject

CommitEvidenceAt(candidate) ==
  \/ \E vote \in commitIntents:
       /\ vote.context = candidate.consumerContext
       /\ vote.view = candidate.view
       /\ vote.subject = candidate.subject
  \/ \E received \in receivedVotes:
       /\ received.node = candidate.node
       /\ received.vote.context = candidate.consumerContext
       /\ received.vote.view = candidate.view
       /\ received.vote.phase = "Commit"
       /\ received.vote.subject = candidate.subject
  \/ \E qc \in commitQCs:
       /\ qc.context = candidate.consumerContext
       /\ qc.view = candidate.view
       /\ qc.subject = candidate.subject

ViewChangeEvidenceAt(candidate) ==
  \/ DurableTimeoutVoteAt(candidate.node, candidate.view)
  \/ \E received \in receivedTimeoutVotes:
       /\ received.node = candidate.node
       /\ received.vote.context = candidate.consumerContext
       /\ received.vote.view = candidate.view
  \/ \E tc \in formedTCs:
       /\ tc.context = candidate.consumerContext
       /\ tc.view >= candidate.view
  \/ \E request \in pendingInstallTC:
       /\ request.node = candidate.node
       /\ request.tc.context = candidate.consumerContext
       /\ request.tc.view >= candidate.view

ExactLeaderEvidenceAt(candidate, rank) ==
  \/ NodeHasDecision(candidate.node)
  \/ (rank[1] = 5 /\ ViewChangeEvidenceAt(candidate))
  \/ (rank[1] = 4
        /\ (ProposalEvidenceAt(candidate)
              \/ PrepareEvidenceAt(candidate)
              \/ CommitEvidenceAt(candidate)))
  \/ (rank[1] = 3
        /\ (PrepareEvidenceAt(candidate)
              \/ CommitEvidenceAt(candidate)))
  \/ (rank[1] = 2 /\ CommitEvidenceAt(candidate))
  \/ (rank[1] = 1 /\ NodeHasDecision(candidate.node))

(***************************************************************************
Authenticated discard provenance is not phase progress.

The exact candidate field is checked against append-only authentication
history.  Certified responses use their canonical signed-wire identity, whose
route-neutral comparison is itself the model's exact immutable payload
identity.  Successors retain the exact parent item in `evidence`, so the
second arm covers causal work without reconstructing a packet from mutable
view state.

The durable-lineage arms are deliberately one phase behind the candidate
rank.  They explain why a disabled candidate exists, but they are not added
to `ExactLeaderEvidenceAt` and never constitute a lower rank or mode goal.
***************************************************************************)
AuthenticatedLeaderDiscardProvenance(candidate) ==
  \/ /\ candidate.item \in AsyncNetworkItems
        /\ IngressItemHasAuthenticatedHistory(candidate.item)
  \/ /\ candidate.evidence \in AsyncNetworkItems
        /\ IngressItemHasAuthenticatedHistory(candidate.evidence)

PriorPhaseLeaderDiscardProvenance(candidate, rank) ==
  \/ /\ rank[1] = 3
        /\ ProposalEvidenceAt(candidate)
  \/ /\ rank[1] = 2
        /\ (ProposalEvidenceAt(candidate)
              \/ PrepareEvidenceAt(candidate))
  \/ /\ rank[1] = 1
        /\ (ProposalEvidenceAt(candidate)
              \/ PrepareEvidenceAt(candidate)
              \/ CommitEvidenceAt(candidate))

ExactLeaderDiscardProvenanceAt(candidate, rank) ==
  \/ AuthenticatedLeaderDiscardProvenance(candidate)
  \/ PriorPhaseLeaderDiscardProvenance(candidate, rank)

\* A down process owns no executable scheduler turn.  This is a preservation
\* classification only; it is excluded from every post-GST progress target.
ExactLeaderSchedulerParked(candidate) ==
  candidate.node \notin up

SelectedSuccessfulLeaderExecution(candidate) ==
  \/ \E node \in ValidatorIds:
       /\ NodeQueueNonempty(node)
       /\ NextNodeCommand(node) = candidate
       /\ CommandDispatchable(candidate)
       /\ FifoRuntimeStep(node)
  \/ \E node \in ValidatorIds:
       /\ DeferredWorkServiceable(node)
       /\ NextDeferredCommand(node) = candidate
       /\ CommandDispatchable(candidate)
       /\ DeferredDrainStep(node)

SameConsumerLeaderDiscard(candidate) ==
  /\ CandidateConsumerCurrent(candidate)'
  /\ candidate.node \in up
  /\ \/ \E node \in ValidatorIds:
          /\ NodeQueueNonempty(node)
          /\ NextNodeCommand(node) = candidate
          /\ ~CommandDispatchable(candidate)
          /\ NodeIdle(node)
          /\ FifoRuntimeStep(node)
     \/ \E node \in ValidatorIds:
          /\ DeferredWorkServiceable(node)
          /\ NextDeferredCommand(node) = candidate
          /\ ~CommandDispatchable(candidate)
          /\ DeferredDrainStep(node)

TerminalLeaderExit(candidate) ==
  \/ NodeHasDecision(candidate.node)'
  \/ NodeHasApplication(candidate.node)'

PacemakerRetiredLeaderOwner(candidate) ==
  /\ ~CandidateConsumerCurrent(candidate)'
  /\ \/ nodeView'[candidate.node] > candidate.consumerView
     \/ generation'[candidate.node] >
          candidate.consumerGeneration

ExecutionProducedLeaderMilestone(candidate, rank) ==
  /\ SelectedSuccessfulLeaderExecution(candidate)
  /\ ExactLeaderCandidatePostMilestone(candidate, rank)'

(***************************************************************************
`SignVote` command identity omits the vote phase.  Unless an invariant
excludes simultaneous Prepare and Commit requests at the same
node/view/subject, `ExecuteSignVote` may consume the request opposite to the
semantic rank by which the candidate was selected.

Here `SerializedBusyOwnershipInvariant` closes that seam: every sign request
is in `SerializedBusyOwners`, whose records are unique by node.  A matching
Prepare request and matching Commit request would be distinct records for the
same node.  Thus the aggregate two-phase publication theorem can be narrowed
to the exact semantic rank without adding a fairness assumption.
***************************************************************************)
CrossPhaseVoteSignAlias(candidate) ==
  /\ candidate.kind = "SignVote"
  /\ MatchingVoteSignRequest(candidate, "Prepare")
  /\ MatchingVoteSignRequest(candidate, "Commit")

THEOREM StrongTypeExcludesCrossPhaseVoteSignAlias ==
  \A candidate:
    AsyncStrongTypeInvariant => ~CrossPhaseVoteSignAlias(candidate)
BY Isa
   DEF CrossPhaseVoteSignAlias, MatchingVoteSignRequest,
       AsyncStrongTypeInvariant, AsyncSerializedBusyKernelInvariant,
       SerializedBusyOwnershipInvariant, SerializedBusyOwners,
       RequestsUniqueByNode

THEOREM RankedSignVoteExecutesItsExactPhase ==
  \A candidate, rank:
    /\ AsyncStrongTypeInvariant
    /\ ExactLeaderCandidateRank(candidate, rank)
    /\ candidate.kind = "SignVote"
    /\ ExecuteSignVote(candidate)
    => ExactLeaderCandidatePostMilestone(candidate, rank)'
BY StrongTypeExcludesCrossPhaseVoteSignAlias,
   ExecuteSignVoteCreatesPhaseExactWireMilestone, Isa
   DEF CrossPhaseVoteSignAlias, ExactLeaderCandidateRank,
       ExactLeaderCandidatePostMilestone, MatchingVoteSignRequest,
       ExecuteSignVote, CommandMatches

CoveredSameConsumerLeaderDiscard(candidate, rank) ==
  /\ SameConsumerLeaderDiscard(candidate)
  /\ \/ ExactLeaderCandidatePostMilestone(candidate, rank)'
     \/ SameIdentityLeaderOwner(candidate)'
     \/ ExactLeaderEvidenceAt(candidate, rank)'
     \/ ExactLeaderDiscardProvenanceAt(candidate, rank)'

UnexplainedSameConsumerLeaderDiscard(candidate, rank) ==
  /\ SameConsumerLeaderDiscard(candidate)
  /\ ~ExactLeaderCandidatePostMilestone(candidate, rank)'
  /\ ~SameIdentityLeaderOwner(candidate)'
  /\ ~ExactLeaderEvidenceAt(candidate, rank)'
  /\ ~ExactLeaderDiscardProvenanceAt(candidate, rank)'

ExactLeaderOwnerExitStep(candidate, rank) ==
  /\ gst
  /\ ExactLeaderCandidateRank(candidate, rank)
  /\ AsyncNext
  /\ ~ResponsiveProtectedCandidateOwned(candidate)'

THEOREM CurrentConsumerRetirementAdvancesPacemakerCoordinate ==
  \A candidate \in AsyncCandidateSet:
    /\ AsyncStrongTypeInvariant
    /\ AsyncNext
    /\ CandidateConsumerCurrent(candidate)
    /\ ~CandidateConsumerCurrent(candidate)'
    => \/ nodeView'[candidate.node] > candidate.consumerView
       \/ generation'[candidate.node] >
            candidate.consumerGeneration
BY AsyncNextAdvancesNodeViews,
   CoreBracketNextDoesNotDecreaseGeneration, Isa
   DEF AsyncStrongTypeInvariant, StrongInductiveInvariant,
       Safety, CandidateConsumerCurrent, AsyncNext, vars

THEOREM RankedLeaderOwnerExitIsExecutionDiscardPacemakerOrTerminal ==
  \A candidate, rank:
    /\ AsyncStrongTypeInvariant
    /\ ExactLeaderOwnerExitStep(candidate, rank)
    => \/ TerminalLeaderExit(candidate)
       \/ PacemakerRetiredLeaderOwner(candidate)
       \/ SelectedSuccessfulLeaderExecution(candidate)
       \/ SameConsumerLeaderDiscard(candidate)
BY CurrentConsumerRetirementAdvancesPacemakerCoordinate, Isa
   DEF ExactLeaderOwnerExitStep, TerminalLeaderExit,
       PacemakerRetiredLeaderOwner,
       SelectedSuccessfulLeaderExecution,
       SameConsumerLeaderDiscard,
       ResponsiveProtectedCandidateOwned,
       ProtectedCandidateOwned, CandidateScheduled,
       AsyncNext, AsyncNonCrashStep, AsyncRunnerStep,
       PostGstRunNode, RunNode, RunNodeWork,
       LocalAdmissionStep, IngressDrainStep, SerializedRuntimeStep,
       RuntimeStep, FifoRuntimeStep, DeferredDrainStep,
       DiscardCommand, DeferCommand

THEOREM RankedLeaderOwnerExitDecomposition ==
  \A candidate, rank:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ ExactLeaderOwnerExitStep(candidate, rank)
    => \/ TerminalLeaderExit(candidate)
       \/ PacemakerRetiredLeaderOwner(candidate)
       \/ ExecutionProducedLeaderMilestone(candidate, rank)
       \/ CoveredSameConsumerLeaderDiscard(candidate, rank)
       \/ UnexplainedSameConsumerLeaderDiscard(candidate, rank)
BY RankedLeaderOwnerExitIsExecutionDiscardPacemakerOrTerminal,
   FifoSuccessfulLeaderExecutionSchedulesDeclaredSuccessors,
   DeferredSuccessfulLeaderExecutionSchedulesDeclaredSuccessors,
   ExecuteSignProposalCreatesExactWireMilestone,
   ExecuteSignVoteCreatesPhaseExactWireMilestone,
   RankedSignVoteExecutesItsExactPhase,
   ExecuteFormPrepareQcCreatesExactWireMilestone,
   ExecuteSignTimeoutCreatesExactWireMilestone,
   ExecutePersistInstallCreatesExactPacemakerMilestone,
   ExecutePersistDecisionCreatesExactDecisionMilestone,
   Isa
   DEF ExecutionProducedLeaderMilestone,
       CoveredSameConsumerLeaderDiscard,
       UnexplainedSameConsumerLeaderDiscard,
       ExactLeaderCandidatePostMilestone,
       SelectedSuccessfulLeaderExecution,
       ExactLeaderCandidateRank, CommandSuccessors

(***************************************************************************
The exact candidate-exit residual and its temporal reduction.

This is not the aggregate leader kernel.  It is one exact one-transition
prohibition: an idle current-consumer discard may not erase the last exact
pipeline owner without an exact surviving phase witness.  The possible
phase-erased `SignVote` alias was discharged above from serialized busy-owner
uniqueness.

An under-quorum FormPrepareQC/FormCommitQC attempt is a legitimate disabled
candidate.  Its source may be a Byzantine vote, so no honest durable intent
need exist.  `PrepareEvidenceAt` and `CommitEvidenceAt` therefore retain the
exact recipient-scoped received-vote occurrence above.  Without that
occurrence the candidate can be discarded by a reachable execution while all
of the older intent/QC evidence disjuncts are false.

The remaining proof boundary is candidate origin, not another action
classification.  The scheduler is intentionally executable over every typed
candidate record, while `AsyncStrongTypeInvariant` and
`AsyncProgressOwnershipInvariant` constrain type, capacity, and unique
ownership rather than the semantic source of each queued record.  The exact
readiness invariant below records the missing inductive fact: an idle current
leader-stage owner is executable, is accompanied by another same-identity
owner, or already has exact phase evidence.  It must be established from
initial candidates and preserved by every producer, ingress, causal-successor,
restart, and removal action.  No theorem in this module assumes the desired
exit-safety conclusion in place of that source-preservation proof.
***************************************************************************)

ExactLeaderSchedulerOriginProvenanceInvariant ==
  \A candidate \in AsyncCandidateSet,
     rank \in (1..5) \X Nat:
    ExactLeaderCandidateRank(candidate, rank)
      => \/ CommandExecutionReady(candidate)
         \/ DispatchableSameIdentityLeaderOwner(candidate)
         \/ ExactLeaderEvidenceAt(candidate, rank)
         \/ ExactLeaderDiscardProvenanceAt(candidate, rank)
         \/ ExactLeaderSchedulerParked(candidate)

ExactLeaderSchedulerIdleReadinessInvariant ==
  \A candidate \in AsyncCandidateSet,
     rank \in (1..5) \X Nat:
    /\ ExactLeaderCandidateRank(candidate, rank)
    /\ NodeIdle(candidate.node)
    => \/ CommandDispatchable(candidate)
       \/ DispatchableSameIdentityLeaderOwner(candidate)
       \/ ExactLeaderEvidenceAt(candidate, rank)
       \/ ExactLeaderDiscardProvenanceAt(candidate, rank)
       \/ ExactLeaderSchedulerParked(candidate)

ExactLeaderSchedulerOriginReadinessInvariant ==
  /\ ExactLeaderSchedulerOriginProvenanceInvariant
  /\ ExactLeaderSchedulerIdleReadinessInvariant

ExactLeaderSchedulerOriginReadinessProperty(specification) ==
  specification => []ExactLeaderSchedulerOriginReadinessInvariant

THEOREM ResponsivePostGstExactLeaderOwnerCannotRemainParked ==
  \A candidate, rank:
    /\ AsyncStrongTypeInvariant
    /\ gst
    /\ ExactLeaderCandidateRank(candidate, rank)
    => ~ExactLeaderSchedulerParked(candidate)
PROOF
  <1>1. ASSUME NEW candidate,
                NEW rank,
                AsyncStrongTypeInvariant,
                gst,
                ExactLeaderCandidateRank(candidate, rank)
         PROVE ~ExactLeaderSchedulerParked(candidate)
    <2>1. /\ AsyncRecoveryTypeInvariant
           /\ AsyncGstRecoveryPhaseInvariant
      BY <1>1 DEF AsyncStrongTypeInvariant
    <2>2. Responsive \subseteq up
      BY <1>1, <2>1, GstResponsiveNodesAreUp
    <2>3. candidate.node \in Responsive
      BY <1>1
         DEF ExactLeaderCandidateRank,
             ResponsiveProtectedCandidateOwned,
             AsyncCurrentResponsiveVoters
    <2> QED BY <2>2, <2>3 DEF ExactLeaderSchedulerParked
  <1> QED BY <1>1

THEOREM CandidateScheduledIsExactFourCarrierUnion ==
  \A candidate:
    CandidateScheduled(candidate)
      <=> candidate \in ActiveScheduledCandidates
BY Isa
   DEF CandidateScheduled, CandidateScheduledIn,
       ActiveScheduledCandidates, QueuedCandidates,
       DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates

THEOREM AsyncInitActiveCandidatesAreInitialCausalCandidates ==
  \A initialContext:
    AsyncInitAt(initialContext)
      => \A candidate \in ActiveScheduledCandidates:
           \E node \in ValidatorIds:
             candidate = InitialCausalCandidate(node)
BY Isa
   DEF AsyncInitAt, AsyncBaseInitAt, AsyncRuntimeInit,
       AsyncIoInit, AsyncDeferredInit,
       ActiveScheduledCandidates, QueuedCandidates,
       DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates,
       InitialCausalCandidate, SequenceSet

THEOREM InitialCausalCandidateExactIdentity ==
  \A node:
    /\ InitialCausalCandidate(node).kind = "AssembleBody"
    /\ InitialCausalCandidate(node).node = node
    /\ InitialCausalCandidate(node).consumerContext = context
    /\ InitialCausalCandidate(node).view = nodeView[node]
BY DEF InitialCausalCandidate, NoItemCandidate,
       AsyncCandidate, AsyncCandidateWithIdentity

THEOREM InitialCausalCandidateExactLeaderRankShape ==
  \A node, rank:
    ExactLeaderCandidateRank(InitialCausalCandidate(node), rank)
      => /\ rank = ProposalSemanticRank(9)
         /\ node = Leader(context, nodeView[node])
         /\ node \in AsyncCurrentResponsiveVoters
         /\ CandidateConsumerCurrent(InitialCausalCandidate(node))
PROOF
  <1>1. ASSUME NEW node,
                NEW rank,
                ExactLeaderCandidateRank(
                  InitialCausalCandidate(node), rank)
         PROVE /\ rank = ProposalSemanticRank(9)
               /\ node = Leader(context, nodeView[node])
               /\ node \in AsyncCurrentResponsiveVoters
               /\ CandidateConsumerCurrent(
                    InitialCausalCandidate(node))
    <2>1. /\ InitialCausalCandidate(node).kind = "AssembleBody"
           /\ InitialCausalCandidate(node).node = node
           /\ InitialCausalCandidate(node).consumerContext = context
           /\ InitialCausalCandidate(node).view = nodeView[node]
      BY InitialCausalCandidateExactIdentity
    <2>2. /\ rank = ProposalSemanticRank(9)
           /\ InitialCausalCandidate(node).node =
                Leader(InitialCausalCandidate(node).consumerContext,
                       InitialCausalCandidate(node).view)
      BY <1>1, <2>1, IsaT(60) DEF ExactLeaderCandidateRank
    <2>3. /\ ResponsiveProtectedCandidateOwned(
                InitialCausalCandidate(node))
           /\ CandidateConsumerCurrent(
                InitialCausalCandidate(node))
      BY <1>1 DEF ExactLeaderCandidateRank
    <2>4. node \in AsyncCurrentResponsiveVoters
      BY <2>1, <2>3 DEF ResponsiveProtectedCandidateOwned
    <2> QED BY <2>1, <2>2, <2>3, <2>4
  <1> QED BY <1>1

THEOREM AsyncInitTypesInitialCausalCandidate ==
  \A initialContext:
    AsyncInitAt(initialContext)
      => \A node \in ValidatorIds:
           AsyncCandidateTyped(InitialCausalCandidate(node))
BY AsyncInitEstablishesStrongTypeInvariant,
   InitialCausalCandidateIsTyped, Isa
   DEF AsyncStrongTypeInvariant, StrongInductiveInvariant, Safety

THEOREM AsyncInitInitialProposalSubjectIsValid ==
  \A initialContext:
    AsyncInitAt(initialContext)
      => \A node \in ValidatorIds:
           /\ AsyncProposalSubject(node) = AsyncHeartbeatSubject
           /\ AsyncProposalSubject(node) \in ValidSubjects
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncInitAt(initialContext),
                NEW node \in ValidatorIds
         PROVE /\ AsyncProposalSubject(node) = AsyncHeartbeatSubject
               /\ AsyncProposalSubject(node) \in ValidSubjects
    <2>1. /\ ModelConfiguration
           /\ highestRank[node] = NoRank
      BY <1>1 DEF AsyncInitAt, AsyncBaseInitAt, InitAt
    <2>2. AsyncProposalSubject(node) = AsyncHeartbeatSubject
      BY <2>1 DEF AsyncProposalSubject
    <2>3. AsyncHeartbeatSubject \in ValidSubjects
      BY <2>1, AsyncHeartbeatSubjectIsValid
    <2> QED BY <2>2, <2>3
  <1> QED BY <1>1

THEOREM AsyncInitHasNoCurrentDurableBody ==
  \A initialContext:
    AsyncInitAt(initialContext)
      => \A node \in ValidatorIds,
            roundView \in Views,
            subject \in Subjects:
           ~BodyHeldBy(durableBodies, node, context,
                       roundView, subject)
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncInitAt(initialContext),
                NEW node \in ValidatorIds,
                NEW roundView \in Views,
                NEW subject \in Subjects
         PROVE ~BodyHeldBy(durableBodies, node, context,
                           roundView, subject)
    <2>1. /\ context = initialContext
           /\ initialContext.height \in Nat
           /\ durableBodies =
                IF initialContext.height = 0
                THEN {}
                ELSE BootstrapParentBodies(initialContext)
      BY <1>1, FrozenContextFieldsTyped
         DEF AsyncInitAt, AsyncBaseInitAt, InitAt, Heights
    <2>2. initialContext.height = 0
             \/ initialContext.height > 0
      BY <2>1, SMT
    <2>3. CASE initialContext.height = 0
      BY <1>1, <2>1, <2>3 DEF BodyHeldBy
    <2>4. CASE initialContext.height > 0
      <3>1. BootstrapParentContext(initialContext) # initialContext
        BY <1>1, <2>4, BootstrapParentContextPrecedes
           DEF AsyncInitAt, AsyncBaseInitAt, InitAt
      <3>2. ASSUME BodyHeldBy(durableBodies, node, context,
                              roundView, subject)
             PROVE FALSE
        <4>1. BodyRecord(node, context, roundView, subject)
                 \in BootstrapParentBodies(initialContext)
          BY <2>1, <2>4, <3>2 DEF BodyHeldBy
        <4>2. context = BootstrapParentContext(initialContext)
          BY <4>1, Isa DEF BootstrapParentBodies, BodyRecord
        <4> QED BY <2>1, <3>1, <4>2
      <3> QED BY <3>2
    <2> QED BY <2>2, <2>3, <2>4
  <1> QED BY <1>1

THEOREM AsyncInitHasNoCurrentDecision ==
  \A initialContext:
    AsyncInitAt(initialContext)
      => \A decision \in decisions:
           decision.qc.context # context
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncInitAt(initialContext),
                NEW decision \in decisions
         PROVE decision.qc.context # context
    <2>1. /\ context = initialContext
           /\ initialContext.height \in Nat
           /\ (initialContext.height = 0 => decisions = {})
           /\ (initialContext.height > 0
                 => /\ decisions =
                          {BootstrapParentDecision(initialContext)}
                    /\ BootstrapParentContext(initialContext)
                         # initialContext)
      BY <1>1, FrozenContextFieldsTyped,
         BootstrapParentContextPrecedes, Isa
         DEF AsyncInitAt, AsyncBaseInitAt, InitAt, Heights
    <2>2. CASE initialContext.height = 0
      BY <1>1, <2>1, <2>2
    <2>3. CASE initialContext.height > 0
      <3>1. decision = BootstrapParentDecision(initialContext)
        BY <1>1, <2>1, <2>3
      <3>2. decision.qc.context =
               BootstrapParentContext(initialContext)
        BY <3>1
           DEF BootstrapParentDecision, BootstrapParentCommitQC, QC
      <3> QED BY <2>1, <3>2
    <2> QED BY <2>1, <2>2, <2>3, SMT
  <1> QED BY <1>1

THEOREM AsyncInitHasNoCurrentLocalBodyOrDecision ==
  \A initialContext:
    AsyncInitAt(initialContext)
      => \A node \in ValidatorIds,
            roundView \in Views,
            subject \in Subjects:
           /\ ~BodyHeldBy(durableBodies, node, context,
                          roundView, subject)
           /\ LocalBodyNotSupersededByDecision(
                node, roundView, subject)
BY AsyncInitHasNoCurrentDurableBody,
   AsyncInitHasNoCurrentDecision, Isa
   DEF LocalBodyNotSupersededByDecision

THEOREM AsyncInitEstablishesAssemblyEnvironment ==
  \A initialContext:
    AsyncInitAt(initialContext)
      => /\ up = ValidatorIds
         /\ Responsive \subseteq Honest
         /\ TypeInvariant
BY AsyncInitEstablishesStrongTypeInvariant, Isa
   DEF AsyncInitAt, AsyncBaseInitAt, InitAt,
       AsyncStrongTypeInvariant, StrongInductiveInvariant,
       Safety, ModelConfiguration

THEOREM TypeInvariantTypesLocalAssemblyRecords ==
  \A node \in ValidatorIds,
     subject \in Subjects:
    TypeInvariant
      => /\ BodyRecord(node, context, nodeView[node], subject)
              \in BodyRecordSet
         /\ ValidationRecord(node, context, nodeView[node],
                             generation[node], subject)
              \in ValidationRecordSet
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                NEW subject \in Subjects,
                TypeInvariant
         PROVE /\ BodyRecord(node, context, nodeView[node], subject)
                    \in BodyRecordSet
               /\ ValidationRecord(node, context, nodeView[node],
                                   generation[node], subject)
                    \in ValidationRecordSet
    <2>1. /\ context \in ContextRecords
           /\ nodeView[node] \in Views
           /\ generation[node] \in Generations
      BY <1>1, Isa DEF TypeInvariant
    <2>2. BodyRecord(node, context, nodeView[node], subject)
             \in BodyRecordSet
      BY <1>1, <2>1, BodyRecordConstructorTyped
    <2>3. ValidationRecord(node, context, nodeView[node],
                           generation[node], subject)
             \in ValidationRecordSet
      BY <1>1, <2>1, ValidationRecordConstructorTyped
    <2> QED BY <2>2, <2>3
  <1> QED BY <1>1

THEOREM AsyncInitInitialLeaderAssemblyGuards ==
  \A initialContext:
    AsyncInitAt(initialContext)
      => \A node \in ValidatorIds,
            rank \in (1..5) \X Nat:
           ExactLeaderCandidateRank(
             InitialCausalCandidate(node), rank)
             => /\ node \in Honest \cap up \cap CurrentVoters
                /\ node = Leader(context, nodeView[node])
                /\ AsyncProposalSubject(node) \in ValidSubjects
                /\ LocalBodyNotSupersededByDecision(
                     node, nodeView[node],
                     AsyncProposalSubject(node))
                /\ BodyRecord(
                     node, context, nodeView[node],
                     AsyncProposalSubject(node)) \in BodyRecordSet
                /\ ValidationRecord(
                     node, context, nodeView[node],
                     generation[node], AsyncProposalSubject(node))
                     \in ValidationRecordSet
                /\ ~BodyHeldBy(
                     durableBodies, node, context, nodeView[node],
                     AsyncProposalSubject(node))
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncInitAt(initialContext),
                NEW node \in ValidatorIds,
                NEW rank \in (1..5) \X Nat,
                ExactLeaderCandidateRank(
                  InitialCausalCandidate(node), rank)
         PROVE /\ node \in Honest \cap up \cap CurrentVoters
               /\ node = Leader(context, nodeView[node])
               /\ AsyncProposalSubject(node) \in ValidSubjects
               /\ LocalBodyNotSupersededByDecision(
                    node, nodeView[node],
                    AsyncProposalSubject(node))
               /\ BodyRecord(
                    node, context, nodeView[node],
                    AsyncProposalSubject(node)) \in BodyRecordSet
               /\ ValidationRecord(
                    node, context, nodeView[node],
                    generation[node], AsyncProposalSubject(node))
                    \in ValidationRecordSet
               /\ ~BodyHeldBy(
                    durableBodies, node, context, nodeView[node],
                    AsyncProposalSubject(node))
    <2>1. /\ node = Leader(context, nodeView[node])
           /\ node \in Responsive \cap CurrentVoters
      BY <1>1, InitialCausalCandidateExactLeaderRankShape
    <2>2. /\ up = ValidatorIds
           /\ Responsive \subseteq Honest
           /\ TypeInvariant
      BY <1>1, AsyncInitEstablishesAssemblyEnvironment
    <2>3. node \in Honest \cap up \cap CurrentVoters
      BY <1>1, <2>1, <2>2, Isa
    <2>4. /\ AsyncProposalSubject(node) \in ValidSubjects
           /\ AsyncProposalSubject(node) \in Subjects
      BY <1>1, <2>2, AsyncInitInitialProposalSubjectIsValid, Isa
         DEF TypeInvariant, ModelConfiguration
    <2>5. /\ nodeView[node] \in Views
           /\ generation[node] \in Generations
      BY <1>1, <2>2, Isa DEF TypeInvariant
    <2>6. /\ LocalBodyNotSupersededByDecision(
                node, nodeView[node], AsyncProposalSubject(node))
           /\ ~BodyHeldBy(
                durableBodies, node, context, nodeView[node],
                AsyncProposalSubject(node))
      BY <1>1, <2>4, <2>5,
         AsyncInitHasNoCurrentLocalBodyOrDecision
    <2>7. /\ BodyRecord(
                node, context, nodeView[node],
                AsyncProposalSubject(node)) \in BodyRecordSet
           /\ ValidationRecord(
                node, context, nodeView[node],
                generation[node], AsyncProposalSubject(node))
                \in ValidationRecordSet
      BY <1>1, <2>2, <2>4,
         TypeInvariantTypesLocalAssemblyRecords
    <2> QED BY <2>1, <2>3, <2>4, <2>6, <2>7
  <1> QED BY <1>1

THEOREM AsyncInitInitialLeaderAssembleBodyIsReady ==
  \A initialContext:
    AsyncInitAt(initialContext)
      => \A node \in ValidatorIds,
            rank \in (1..5) \X Nat:
           ExactLeaderCandidateRank(
             InitialCausalCandidate(node), rank)
             => AssembleLocalBodyReady(
                  node, AsyncProposalSubject(node))
BY AsyncInitInitialLeaderAssemblyGuards
   DEF AssembleLocalBodyReady

THEOREM InitialCausalCandidateDispatchShape ==
  \A node:
    /\ InitialCausalCandidate(node).class = "Normal"
    /\ InitialCausalCandidate(node).kind = "AssembleBody"
    /\ InitialCausalCandidate(node).node = node
    /\ InitialCausalCandidate(node).view = nodeView[node]
    /\ InitialCausalCandidate(node).subject =
         AsyncProposalSubject(node)
    /\ InitialCausalCandidate(node).item = NoAsyncItem
    /\ CommandMatches(
         InitialCausalCandidate(node), node, nodeView[node],
         AsyncProposalSubject(node))
    /\ CandidateConsumerCurrent(InitialCausalCandidate(node))
    /\ LocalAssemblyBusyDispatchAllowed(
         InitialCausalCandidate(node))
BY DEF InitialCausalCandidate, NoItemCandidate,
       AsyncCandidate, AsyncCandidateWithIdentity,
       CommandMatches, CandidateConsumerCurrent,
       LocalAssemblyBusyDispatchAllowed

RegularAssembleCommandReady(command) ==
  /\ command.kind = "AssembleBody"
  /\ CommandMatches(
       command, command.node, nodeView[command.node], command.subject)
  /\ AssembleLocalBodyReady(command.node, command.subject)

THEOREM RegularAssembleCommandReadyProjectsRegularCore ==
  \A command:
    RegularAssembleCommandReady(command)
      => RegularCoreCommandReady(command)
BY DEF RegularAssembleCommandReady, RegularCoreCommandReady

THEOREM AsyncInitInitialLeaderRegularAssembleCommandReady ==
  \A initialContext:
    AsyncInitAt(initialContext)
      => \A node \in ValidatorIds,
            rank \in (1..5) \X Nat:
           ExactLeaderCandidateRank(
             InitialCausalCandidate(node), rank)
             => RegularAssembleCommandReady(
                  InitialCausalCandidate(node))
BY InitialCausalCandidateDispatchShape,
   AsyncInitInitialLeaderAssembleBodyIsReady, Isa
   DEF RegularAssembleCommandReady

THEOREM AsyncInitInitialLeaderRegularCoreCommandReady ==
  \A initialContext:
    AsyncInitAt(initialContext)
      => \A node \in ValidatorIds,
            rank \in (1..5) \X Nat:
           ExactLeaderCandidateRank(
             InitialCausalCandidate(node), rank)
             => RegularCoreCommandReady(
                  InitialCausalCandidate(node))
BY AsyncInitInitialLeaderRegularAssembleCommandReady,
   RegularAssembleCommandReadyProjectsRegularCore

THEOREM RegularCoreCommandReadyProjectsExecuteRegular ==
  \A command:
    RegularCoreCommandReady(command)
      => ExecuteRegularCommandReady(command)
BY DEF ExecuteRegularCommandReady

THEOREM ExecuteRegularCommandReadyProjectsCommandExecution ==
  \A command:
    ExecuteRegularCommandReady(command)
      => CommandExecutionReady(command)
BY DEF CommandExecutionReady

THEOREM AsyncInitInitialLeaderCommandExecutionReady ==
  \A initialContext:
    AsyncInitAt(initialContext)
      => \A node \in ValidatorIds,
            rank \in (1..5) \X Nat:
           ExactLeaderCandidateRank(
             InitialCausalCandidate(node), rank)
             => CommandExecutionReady(
                  InitialCausalCandidate(node))
BY AsyncInitInitialLeaderRegularCoreCommandReady,
   RegularCoreCommandReadyProjectsExecuteRegular,
   ExecuteRegularCommandReadyProjectsCommandExecution

THEOREM AsyncInitialLeaderCandidateIsReadyOrHasProposalEvidence ==
  \A initialContext:
    AsyncInitAt(initialContext)
      => \A node \in ValidatorIds,
            rank \in (1..5) \X Nat:
           /\ ExactLeaderCandidateRank(
                InitialCausalCandidate(node), rank)
           /\ NodeIdle(node)
           => \/ CommandDispatchable(InitialCausalCandidate(node))
              \/ ExactLeaderEvidenceAt(
                   InitialCausalCandidate(node), rank)
BY InitialCausalCandidateExactLeaderRankShape,
   AsyncInitTypesInitialCausalCandidate,
   InitialCausalCandidateDispatchShape,
   AsyncInitInitialLeaderCommandExecutionReady, Isa
   DEF CommandDispatchable

THEOREM ExactLeaderCandidateRankIsScheduled ==
  \A candidate, rank:
    ExactLeaderCandidateRank(candidate, rank)
      => CandidateScheduled(candidate)
BY ExactLeaderCandidateRankIsSemanticRank, Isa
   DEF ResponsiveProtectedCandidateOwned,
       ProtectedCandidateOwned

THEOREM ExactLeaderOriginProvenanceImpliesIdleReadiness ==
  /\ ExactLeaderSchedulerOriginProvenanceInvariant
  /\ AsyncStrongTypeInvariant
  => ExactLeaderSchedulerIdleReadinessInvariant
PROOF
  <1>1. ASSUME ExactLeaderSchedulerOriginProvenanceInvariant,
                AsyncStrongTypeInvariant
         PROVE ExactLeaderSchedulerIdleReadinessInvariant
    <2>1. ASSUME NEW candidate \in AsyncCandidateSet,
                  NEW rank \in (1..5) \X Nat,
                  /\ ExactLeaderCandidateRank(candidate, rank)
                     /\ NodeIdle(candidate.node)
           PROVE \/ CommandDispatchable(candidate)
                 \/ DispatchableSameIdentityLeaderOwner(candidate)
                 \/ ExactLeaderEvidenceAt(candidate, rank)
                 \/ ExactLeaderDiscardProvenanceAt(candidate, rank)
                 \/ ExactLeaderSchedulerParked(candidate)
      <3>1. \/ CommandExecutionReady(candidate)
             \/ DispatchableSameIdentityLeaderOwner(candidate)
             \/ ExactLeaderEvidenceAt(candidate, rank)
             \/ ExactLeaderDiscardProvenanceAt(candidate, rank)
             \/ ExactLeaderSchedulerParked(candidate)
        BY <1>1, <2>1
           DEF ExactLeaderSchedulerOriginProvenanceInvariant
      <3>2. AsyncTypeInvariant
        BY <1>1, AsyncStrongTypeProjectsAsyncType
      <3>3. candidate \in ActiveScheduledCandidates
        BY <2>1, ExactLeaderCandidateRankIsScheduled,
           CandidateScheduledIsExactFourCarrierUnion
      <3>4. AsyncCandidateTyped(candidate)
        BY <3>2, <3>3, ActiveScheduledCandidatesAreTyped
      <3>5. CandidateConsumerCurrent(candidate)
        BY <2>1 DEF ExactLeaderCandidateRank
      <3>6. CommandExecutionReady(candidate)
               => CommandDispatchable(candidate)
        BY <2>1, <3>4, <3>5 DEF CommandDispatchable
      <3> QED BY <3>1, <3>6
    <2> QED BY <2>1
         DEF ExactLeaderSchedulerIdleReadinessInvariant
  <1> QED BY <1>1

THEOREM AsyncInitRankedLeaderCandidateIsInitialCausalCandidate ==
  \A initialContext:
    AsyncInitAt(initialContext)
      => \A candidate \in AsyncCandidateSet,
            rank \in (1..5) \X Nat:
           ExactLeaderCandidateRank(candidate, rank)
             => \E node \in ValidatorIds:
                  candidate = InitialCausalCandidate(node)
BY ExactLeaderCandidateRankIsScheduled,
   CandidateScheduledIsExactFourCarrierUnion,
   AsyncInitActiveCandidatesAreInitialCausalCandidates

THEOREM AsyncInitEstablishesExactLeaderSchedulerOriginProvenance ==
  \A initialContext:
    AsyncInitAt(initialContext)
      => ExactLeaderSchedulerOriginProvenanceInvariant
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncInitAt(initialContext)
         PROVE ExactLeaderSchedulerOriginProvenanceInvariant
    <2>1. ASSUME NEW candidate \in AsyncCandidateSet,
                  NEW rank \in (1..5) \X Nat,
                  ExactLeaderCandidateRank(candidate, rank)
           PROVE \/ CommandExecutionReady(candidate)
                 \/ DispatchableSameIdentityLeaderOwner(candidate)
                 \/ ExactLeaderEvidenceAt(candidate, rank)
                 \/ ExactLeaderDiscardProvenanceAt(candidate, rank)
                 \/ ExactLeaderSchedulerParked(candidate)
      <3>1. \E node \in ValidatorIds:
               candidate = InitialCausalCandidate(node)
        BY <1>1, <2>1,
           AsyncInitRankedLeaderCandidateIsInitialCausalCandidate
      <3>2. \A node \in ValidatorIds:
               candidate = InitialCausalCandidate(node)
                 => CommandExecutionReady(candidate)
        <4>1. ASSUME NEW node \in ValidatorIds,
                      candidate = InitialCausalCandidate(node)
               PROVE CommandExecutionReady(candidate)
          <5>1. /\ ExactLeaderCandidateRank(
                       InitialCausalCandidate(node), rank)
            BY <2>1, <4>1
          <5> QED BY <1>1, <4>1, <5>1,
                     AsyncInitInitialLeaderCommandExecutionReady
        <4> QED BY <4>1
      <3> QED BY <3>1, <3>2
    <2> QED BY <2>1
         DEF ExactLeaderSchedulerOriginProvenanceInvariant
  <1> QED BY <1>1

THEOREM AsyncInitEstablishesExactLeaderSchedulerOriginReadiness ==
  \A initialContext:
    AsyncInitAt(initialContext)
      => ExactLeaderSchedulerOriginReadinessInvariant
BY AsyncInitEstablishesExactLeaderSchedulerOriginProvenance,
   AsyncInitEstablishesStrongTypeInvariant,
   ExactLeaderOriginProvenanceImpliesIdleReadiness, Isa
   DEF ExactLeaderSchedulerOriginReadinessInvariant

(***************************************************************************
Preservation starts with the literal four scheduler carriers.  This frame is
intentionally stronger than an informal "does not enqueue work" argument:
every Core field inspected by exact rank, idleness, dispatch readiness, and
semantic evidence is frozen together with every carrier in
`CandidateScheduledIn`.
***************************************************************************)

ExactLeaderSchedulerReadinessFrame ==
  /\ UNCHANGED vars
  /\ UNCHANGED <<asyncCommandQueues,
                 asyncDeferredCompletionQueues,
                 asyncDeferredProgressQueues,
                 asyncDeferredNormalQueues,
                 asyncCausalQueues,
                 asyncOutstandingWork,
                 asyncSentItems,
                 asyncHeldChunks>>

THEOREM ExactLeaderSchedulerReadinessFramePreservesScheduled ==
  \A candidate:
    ExactLeaderSchedulerReadinessFrame
      => (CandidateScheduled(candidate)'
            <=> CandidateScheduled(candidate))
BY IsaT(60)
   DEF ExactLeaderSchedulerReadinessFrame,
       CandidateScheduled, CandidateScheduledIn

THEOREM ExactLeaderSchedulerReadinessFramePreservesCurrentConsumer ==
  \A candidate:
    ExactLeaderSchedulerReadinessFrame
      => (CandidateConsumerCurrent(candidate)'
            <=> CandidateConsumerCurrent(candidate))
BY Isa
   DEF ExactLeaderSchedulerReadinessFrame,
       CandidateConsumerCurrent, vars

THEOREM ExactLeaderSchedulerReadinessFramePreservesMatchingVoteSign ==
  \A candidate, phase:
    ExactLeaderSchedulerReadinessFrame
      => (MatchingVoteSignRequest(candidate, phase)'
            <=> MatchingVoteSignRequest(candidate, phase))
BY Isa
   DEF ExactLeaderSchedulerReadinessFrame,
       MatchingVoteSignRequest, vars

THEOREM ExactLeaderSchedulerReadinessFramePreservesNetworkItems ==
  ExactLeaderSchedulerReadinessFrame
    => UNCHANGED AsyncNetworkItems
BY AsyncNetworkItemsStableUnderContextAndViewFrame, Isa
   DEF ExactLeaderSchedulerReadinessFrame, vars

THEOREM ExactLeaderSchedulerReadinessFramePreservesNetworkItemMembership ==
  \A item:
    ExactLeaderSchedulerReadinessFrame
      => ((item \in AsyncNetworkItems)'
            <=> item \in AsyncNetworkItems)
BY ExactLeaderSchedulerReadinessFramePreservesNetworkItems, Isa

THEOREM ExactLeaderSchedulerReadinessFramePreservesEvidenceSet ==
  ExactLeaderSchedulerReadinessFrame
    => UNCHANGED AsyncEvidenceSet
BY ExactLeaderSchedulerReadinessFramePreservesNetworkItems, Isa
   DEF AsyncEvidenceSet

THEOREM ExactLeaderSchedulerReadinessFramePreservesCandidateSet ==
  ExactLeaderSchedulerReadinessFrame
    => UNCHANGED AsyncCandidateSet
BY ExactLeaderSchedulerReadinessFramePreservesNetworkItems,
   ExactLeaderSchedulerReadinessFramePreservesEvidenceSet, Isa
   DEF AsyncCandidateSet

THEOREM ExactLeaderSchedulerReadinessFramePreservesCandidateMembership ==
  \A candidate:
    ExactLeaderSchedulerReadinessFrame
      => ((candidate \in AsyncCandidateSet)'
            <=> candidate \in AsyncCandidateSet)
BY ExactLeaderSchedulerReadinessFramePreservesCandidateSet, Isa

THEOREM ExactLeaderSchedulerReadinessFramePreservesNoItemProposalPrepare ==
  \A candidate:
    ExactLeaderSchedulerReadinessFrame
      => (NormalProposalPrepareNoItemCandidate(candidate)'
            <=> NormalProposalPrepareNoItemCandidate(candidate))
BY ExactLeaderSchedulerReadinessFramePreservesCandidateSet, Isa
   DEF NormalProposalPrepareNoItemCandidate

THEOREM ExactLeaderSchedulerReadinessFramePreservesNetworkProposalPrepare ==
  \A candidate:
    ExactLeaderSchedulerReadinessFrame
      => (NormalProposalPrepareNetworkCandidate(candidate)'
            <=> NormalProposalPrepareNetworkCandidate(candidate))
BY ExactLeaderSchedulerReadinessFramePreservesNetworkItems, Isa
   DEF NormalProposalPrepareNetworkCandidate

THEOREM ExactLeaderSchedulerReadinessFramePreservesNormalProposalPrepare ==
  \A candidate:
    ExactLeaderSchedulerReadinessFrame
      => (NormalProposalPrepareCandidate(candidate)'
            <=> NormalProposalPrepareCandidate(candidate))
BY ExactLeaderSchedulerReadinessFramePreservesCandidateMembership,
   ExactLeaderSchedulerReadinessFramePreservesNoItemProposalPrepare,
   ExactLeaderSchedulerReadinessFramePreservesNetworkProposalPrepare, Isa
   DEF NormalProposalPrepareCandidate

THEOREM ExactLeaderSchedulerReadinessFramePreservesProtectedService ==
  \A candidate:
    ExactLeaderSchedulerReadinessFrame
      => (ProtectedServiceCandidate(candidate)'
            <=> ProtectedServiceCandidate(candidate))
BY ExactLeaderSchedulerReadinessFramePreservesCandidateMembership,
   ExactLeaderSchedulerReadinessFramePreservesNormalProposalPrepare, Isa
   DEF ProtectedServiceCandidate

THEOREM ExactLeaderSchedulerReadinessFramePreservesProtectedOwnership ==
  \A candidate:
    ExactLeaderSchedulerReadinessFrame
      => (ResponsiveProtectedCandidateOwned(candidate)'
            <=> ResponsiveProtectedCandidateOwned(candidate))
BY ExactLeaderSchedulerReadinessFramePreservesScheduled,
   ExactLeaderSchedulerReadinessFramePreservesProtectedService,
   IsaT(60)
   DEF ExactLeaderSchedulerReadinessFrame,
       ResponsiveProtectedCandidateOwned, ProtectedCandidateOwned,
       AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch,
       NodeHasApplication, vars

THEOREM ExactLeaderSchedulerReadinessFramePreservesViewChangeRank ==
  \A candidate, rank:
    ExactLeaderSchedulerReadinessFrame
      => (ExactLeaderViewChangeRank(candidate, rank)'
            <=> ExactLeaderViewChangeRank(candidate, rank))
OBVIOUS

THEOREM ExactLeaderSchedulerReadinessFramePreservesProposalRank ==
  \A candidate, rank:
    ExactLeaderSchedulerReadinessFrame
      => (ExactLeaderProposalRank(candidate, rank)'
            <=> ExactLeaderProposalRank(candidate, rank))
OBVIOUS

THEOREM ExactLeaderSchedulerReadinessFramePreservesPrepareStaticRank ==
  \A candidate, rank:
    ExactLeaderSchedulerReadinessFrame
      => (ExactLeaderPrepareStaticRank(candidate, rank)'
            <=> ExactLeaderPrepareStaticRank(candidate, rank))
OBVIOUS

THEOREM ExactLeaderSchedulerReadinessFramePreservesPrepareSignRank ==
  \A candidate, rank:
    ExactLeaderSchedulerReadinessFrame
      => (ExactLeaderPrepareSignRank(candidate, rank)'
            <=> ExactLeaderPrepareSignRank(candidate, rank))
BY ExactLeaderSchedulerReadinessFramePreservesMatchingVoteSign, Isa
   DEF ExactLeaderPrepareSignRank

THEOREM ExactLeaderSchedulerReadinessFramePreservesPrepareRank ==
  \A candidate, rank:
    ExactLeaderSchedulerReadinessFrame
      => (ExactLeaderPrepareRank(candidate, rank)'
            <=> ExactLeaderPrepareRank(candidate, rank))
BY ExactLeaderSchedulerReadinessFramePreservesPrepareStaticRank,
   ExactLeaderSchedulerReadinessFramePreservesPrepareSignRank, Isa
   DEF ExactLeaderPrepareRank

THEOREM ExactLeaderSchedulerReadinessFramePreservesCommitStaticRank ==
  \A candidate, rank:
    ExactLeaderSchedulerReadinessFrame
      => (ExactLeaderCommitStaticRank(candidate, rank)'
            <=> ExactLeaderCommitStaticRank(candidate, rank))
OBVIOUS

THEOREM ExactLeaderSchedulerReadinessFramePreservesCommitSignRank ==
  \A candidate, rank:
    ExactLeaderSchedulerReadinessFrame
      => (ExactLeaderCommitSignRank(candidate, rank)'
            <=> ExactLeaderCommitSignRank(candidate, rank))
BY ExactLeaderSchedulerReadinessFramePreservesMatchingVoteSign, Isa
   DEF ExactLeaderCommitSignRank

THEOREM ExactLeaderSchedulerReadinessFramePreservesCommitRank ==
  \A candidate, rank:
    ExactLeaderSchedulerReadinessFrame
      => (ExactLeaderCommitRank(candidate, rank)'
            <=> ExactLeaderCommitRank(candidate, rank))
BY ExactLeaderSchedulerReadinessFramePreservesCommitStaticRank,
   ExactLeaderSchedulerReadinessFramePreservesCommitSignRank, Isa
   DEF ExactLeaderCommitRank

THEOREM ExactLeaderSchedulerReadinessFramePreservesDecisionRank ==
  \A candidate, rank:
    ExactLeaderSchedulerReadinessFrame
      => (ExactLeaderDecisionRank(candidate, rank)'
            <=> ExactLeaderDecisionRank(candidate, rank))
OBVIOUS

THEOREM ExactLeaderSchedulerReadinessFramePreservesPhaseRank ==
  \A candidate, rank:
    ExactLeaderSchedulerReadinessFrame
      => (ExactLeaderPhaseRank(candidate, rank)'
            <=> ExactLeaderPhaseRank(candidate, rank))
BY ExactLeaderSchedulerReadinessFramePreservesViewChangeRank,
   ExactLeaderSchedulerReadinessFramePreservesProposalRank,
   ExactLeaderSchedulerReadinessFramePreservesPrepareRank,
   ExactLeaderSchedulerReadinessFramePreservesCommitRank,
   ExactLeaderSchedulerReadinessFramePreservesDecisionRank, Isa
   DEF ExactLeaderPhaseRank

THEOREM ExactLeaderSchedulerReadinessFramePreservesRank ==
  \A candidate, rank:
    ExactLeaderSchedulerReadinessFrame
      => (ExactLeaderCandidateRank(candidate, rank)'
            <=> ExactLeaderCandidateRank(candidate, rank))
BY ExactLeaderSchedulerReadinessFramePreservesCurrentConsumer,
   ExactLeaderSchedulerReadinessFramePreservesProtectedOwnership,
   ExactLeaderSchedulerReadinessFramePreservesPhaseRank, Isa
   DEF ExactLeaderCandidateRank

THEOREM ExactLeaderSchedulerReadinessFramePreservesPersistDecisionReady ==
  \A candidate:
    ExactLeaderSchedulerReadinessFrame
      => (ExecutePersistDecisionReady(candidate)'
            <=> ExecutePersistDecisionReady(candidate))
BY Isa
   DEF ExactLeaderSchedulerReadinessFrame,
       ExecutePersistDecisionReady, CommandMatches,
       PersistDecisionReady, vars

THEOREM ExactLeaderSchedulerReadinessFramePreservesApplyReady ==
  \A candidate:
    ExactLeaderSchedulerReadinessFrame
      => (ExecuteApplyReady(candidate)'
            <=> ExecuteApplyReady(candidate))
BY Isa
   DEF ExactLeaderSchedulerReadinessFrame,
       ExecuteApplyReady, CommandMatches, ApplyDecisionReady,
       DecisionQcValues, vars

THEOREM ExactLeaderSchedulerReadinessFramePreservesPersistInstallReady ==
  \A candidate:
    ExactLeaderSchedulerReadinessFrame
      => (ExecutePersistInstallReady(candidate)'
            <=> ExecutePersistInstallReady(candidate))
BY ExactLeaderSchedulerReadinessFramePreservesNetworkItems, IsaT(60)
   DEF ExactLeaderSchedulerReadinessFrame,
       ExecutePersistInstallReady, InstallTcEvidenceMatches,
       PersistInstallTCReady, StrictSameRoundTcUpgrade,
       NodeInstalledTC, TcHighRank, vars

THEOREM ExactLeaderSchedulerReadinessFramePreservesSignProposalReady ==
  \A candidate:
    ExactLeaderSchedulerReadinessFrame
      => (ExecuteSignProposalReady(candidate)'
            <=> ExecuteSignProposalReady(candidate))
BY ExactLeaderSchedulerReadinessFramePreservesNetworkItems, IsaT(60)
   DEF ExactLeaderSchedulerReadinessFrame,
       ExecuteSignProposalReady, CommandMatches,
       CompleteProposalSignatureReady, ProposalOutbox,
       CurrentVoters, CurrentEpoch, vars

THEOREM ExactLeaderSchedulerReadinessFramePreservesSignVoteReady ==
  \A candidate:
    ExactLeaderSchedulerReadinessFrame
      => (ExecuteSignVoteReady(candidate)'
            <=> ExecuteSignVoteReady(candidate))
BY ExactLeaderSchedulerReadinessFramePreservesNetworkItems, IsaT(60)
   DEF ExactLeaderSchedulerReadinessFrame,
       ExecuteSignVoteReady, CommandMatches,
       CompleteVoteSignatureReady, VoteRoundAdmissible,
       LockedPrepareRound, VoteOutbox,
       CurrentVoters, CurrentEpoch, vars

THEOREM ExactLeaderSchedulerReadinessFramePreservesFormPrepareQCReady ==
  \A candidate:
    ExactLeaderSchedulerReadinessFrame
      => (ExecuteFormPrepareQCReady(candidate)'
            <=> ExecuteFormPrepareQCReady(candidate))
BY ExactLeaderSchedulerReadinessFramePreservesNetworkItems, IsaT(60)
   DEF ExactLeaderSchedulerReadinessFrame,
       ExecuteFormPrepareQCReady, FormPrepareQCReady,
       VoteSignersAt, QcWireValid, QcOutbox,
       CurrentVoters, CurrentEpoch, vars

THEOREM ExactLeaderSchedulerReadinessFramePreservesSignTimeoutReady ==
  \A candidate:
    ExactLeaderSchedulerReadinessFrame
      => (ExecuteSignTimeoutReady(candidate)'
            <=> ExecuteSignTimeoutReady(candidate))
BY ExactLeaderSchedulerReadinessFramePreservesNetworkItems, IsaT(60)
   DEF ExactLeaderSchedulerReadinessFrame,
       ExecuteSignTimeoutReady, CommandMatches,
       CompleteTimeoutSignatureReady, LocalTimeoutCompletionGuard,
       PendingNodes, NoDecisionForNode, LocalTimeoutVoteFor,
       ExactPrepareQcMatchesRef, PrepareQcOptionWireValid,
       QcWireValid, PrepareQcRank, PrepareQcSubject,
       TimeoutOutbox, CurrentVoters, CurrentEpoch, vars

THEOREM ExactLeaderSchedulerReadinessFramePreservesDecisionFetchReady ==
  \A candidate:
    ExactLeaderSchedulerReadinessFrame
      => (ExecuteDecisionFetchReady(candidate)'
            <=> ExecuteDecisionFetchReady(candidate))
BY IsaT(60)
   DEF ExactLeaderSchedulerReadinessFrame,
       ExecuteDecisionFetchReady, CertifiedRecoveryFetchFrontier,
       DecisionFetchFrontier, LockedPrepareFetchFrontier,
       DecisionQcValues, DecisionCertifiedBodyRecoveryAuthority,
       LockedPrepareRecoverySource, HistoricalLockedPrepareSource,
       HistoricalLockedPrepareRecoveryProvenance,
       InstalledTcSelectsPrepareFor, ExactLockedCommitIntents,
       NoDecisionForNode, vars

THEOREM ExactLeaderSchedulerReadinessFramePreservesRequestCertifiedBodyReady ==
  \A candidate:
    ExactLeaderSchedulerReadinessFrame
      => (ExecuteRequestCertifiedBodyReady(candidate)'
            <=> ExecuteRequestCertifiedBodyReady(candidate))
BY IsaT(60)
   DEF ExactLeaderSchedulerReadinessFrame,
       ExecuteRequestCertifiedBodyReady, CommandMatches,
       BodyHeldBy, DecisionQcValues,
       CertifiedBodyRecoveryAuthority,
       DecisionCertifiedBodyRecoveryAuthority,
       HistoricalLockedPrepareSource,
       HistoricalLockedPrepareRecoveryProvenance,
       InstalledTcSelectsPrepareFor, ExactLockedCommitIntents,
       NoDecisionForNode, CertifiedRequestOutbox,
       CertifiedArchiveRoutes, CurrentVoters, CurrentEpoch, vars

THEOREM ExactLeaderSchedulerReadinessFramePreservesDeliverProposalReady ==
  \A envelope:
    ExactLeaderSchedulerReadinessFrame
      => (DeliverProposalReady(envelope)'
            <=> DeliverProposalReady(envelope))
BY IsaT(60)
   DEF ExactLeaderSchedulerReadinessFrame,
       DeliverProposalReady, ProposalWireValidFor,
       ProposalJustified, SafeToPrepare, TCValid,
       ExactPrepareQcMatchesRef, PrepareQcOptionWireValid,
       QcWireValid, PrepareQcRank, PrepareQcSubject,
       CurrentVoters, CurrentEpoch, vars

THEOREM ExactLeaderSchedulerReadinessFramePreservesDeliverVoteReady ==
  \A envelope:
    ExactLeaderSchedulerReadinessFrame
      => (DeliverVoteReady(envelope)'
            <=> DeliverVoteReady(envelope))
BY IsaT(60)
   DEF ExactLeaderSchedulerReadinessFrame,
       DeliverVoteReady, VoteRoundAdmissible, LockedPrepareRound,
       CurrentVoters, CurrentEpoch, vars

THEOREM ExactLeaderSchedulerReadinessFramePreservesDeliverQCReady ==
  \A envelope:
    ExactLeaderSchedulerReadinessFrame
      => (DeliverQCReady(envelope)'
            <=> DeliverQCReady(envelope))
BY IsaT(60)
   DEF ExactLeaderSchedulerReadinessFrame,
       DeliverQCReady, QcWireValid,
       CurrentVoters, CurrentEpoch, vars

THEOREM ExactLeaderSchedulerReadinessFramePreservesDeliverTimeoutReady ==
  \A envelope:
    ExactLeaderSchedulerReadinessFrame
      => (DeliverTimeoutReady(envelope)'
            <=> DeliverTimeoutReady(envelope))
BY IsaT(60)
   DEF ExactLeaderSchedulerReadinessFrame,
       DeliverTimeoutReady, TimeoutDeliveryGuard, NodeIdle,
       PendingNodes, SigningNodes, ExactPrepareQcMatchesRef,
       PrepareQcOptionWireValid, QcWireValid,
       PrepareQcRank, PrepareQcSubject,
       CurrentVoters, CurrentEpoch, vars

THEOREM ExactLeaderSchedulerReadinessFramePreservesDeliverTCReady ==
  \A envelope:
    ExactLeaderSchedulerReadinessFrame
      => (DeliverTCReady(envelope)'
            <=> DeliverTCReady(envelope))
BY IsaT(60)
   DEF ExactLeaderSchedulerReadinessFrame,
       DeliverTCReady, TCValid,
       ExactPrepareQcMatchesRef, PrepareQcOptionWireValid,
       QcWireValid, PrepareQcRank, PrepareQcSubject,
       CurrentVoters, CurrentEpoch, vars

THEOREM ExactLeaderSchedulerReadinessFramePreservesCoreProposalReady ==
  \A candidate:
    (/\ ExactLeaderSchedulerReadinessFrame
     /\ candidate.kind = "DeliverProposal")
      => (ExecuteCoreDeliveryReady(candidate)'
            <=> ExecuteCoreDeliveryReady(candidate))
BY IsaT(120)
   DEF ExactLeaderSchedulerReadinessFrame,
       ExecuteCoreDeliveryReady, DeliverProposalReady,
       ProposalWireValidFor, ProposalJustified, SafeToPrepare,
       TCValid, ExactPrepareQcMatchesRef,
       PrepareQcOptionWireValid, QcWireValid,
       PrepareQcRank, PrepareQcSubject,
       CurrentVoters, CurrentEpoch, vars

THEOREM ExactLeaderSchedulerReadinessFramePreservesCoreVoteReady ==
  \A candidate:
    (/\ ExactLeaderSchedulerReadinessFrame
     /\ candidate.kind = "DeliverVote")
      => (ExecuteCoreDeliveryReady(candidate)'
            <=> ExecuteCoreDeliveryReady(candidate))
BY IsaT(120)
   DEF ExactLeaderSchedulerReadinessFrame,
       ExecuteCoreDeliveryReady, DeliverVoteReady,
       VoteRoundAdmissible, LockedPrepareRound,
       CurrentVoters, CurrentEpoch, vars

THEOREM ExactLeaderSchedulerReadinessFramePreservesCoreQCReady ==
  \A candidate:
    (/\ ExactLeaderSchedulerReadinessFrame
     /\ candidate.kind = "DeliverQC")
      => (ExecuteCoreDeliveryReady(candidate)'
            <=> ExecuteCoreDeliveryReady(candidate))
BY IsaT(120)
   DEF ExactLeaderSchedulerReadinessFrame,
       ExecuteCoreDeliveryReady, DeliverQCReady,
       QcWireValid, CurrentVoters, CurrentEpoch, vars

THEOREM ExactLeaderSchedulerReadinessFramePreservesCoreTimeoutReady ==
  \A candidate:
    (/\ ExactLeaderSchedulerReadinessFrame
     /\ candidate.kind = "DeliverTimeout")
      => (ExecuteCoreDeliveryReady(candidate)'
            <=> ExecuteCoreDeliveryReady(candidate))
BY IsaT(120)
   DEF ExactLeaderSchedulerReadinessFrame,
       ExecuteCoreDeliveryReady, DeliverTimeoutReady,
       TimeoutDeliveryGuard, NodeIdle, PendingNodes, SigningNodes,
       ExactPrepareQcMatchesRef, PrepareQcOptionWireValid,
       QcWireValid, PrepareQcRank, PrepareQcSubject,
       CurrentVoters, CurrentEpoch, vars

THEOREM ExactLeaderSchedulerReadinessFramePreservesCoreTCReady ==
  \A candidate:
    (/\ ExactLeaderSchedulerReadinessFrame
     /\ candidate.kind = "DeliverTC")
      => (ExecuteCoreDeliveryReady(candidate)'
            <=> ExecuteCoreDeliveryReady(candidate))
BY IsaT(120)
   DEF ExactLeaderSchedulerReadinessFrame,
       ExecuteCoreDeliveryReady, DeliverTCReady, TCValid,
       ExactPrepareQcMatchesRef, PrepareQcOptionWireValid,
       QcWireValid, PrepareQcRank, PrepareQcSubject,
       CurrentVoters, CurrentEpoch, vars

CoreDeliveryCommandKind(candidate) ==
  candidate.kind
    \in {"DeliverProposal", "DeliverVote", "DeliverQC",
         "DeliverTimeout", "DeliverTC"}

THEOREM CoreDeliveryReadyHasDeliveryCommandKind ==
  \A candidate:
    ExecuteCoreDeliveryReady(candidate)
      => CoreDeliveryCommandKind(candidate)
BY Isa
   DEF ExecuteCoreDeliveryReady, CoreDeliveryCommandKind

THEOREM PrimedCoreDeliveryReadyHasDeliveryCommandKind ==
  \A candidate:
    ExecuteCoreDeliveryReady(candidate)'
      => CoreDeliveryCommandKind(candidate)
BY Isa
   DEF ExecuteCoreDeliveryReady, CoreDeliveryCommandKind

THEOREM ExactLeaderSchedulerReadinessFramePreservesCoreDeliveryReady ==
  \A candidate:
    ExactLeaderSchedulerReadinessFrame
      => (ExecuteCoreDeliveryReady(candidate)'
            <=> ExecuteCoreDeliveryReady(candidate))
BY CoreDeliveryReadyHasDeliveryCommandKind,
   PrimedCoreDeliveryReadyHasDeliveryCommandKind,
   ExactLeaderSchedulerReadinessFramePreservesCoreProposalReady,
   ExactLeaderSchedulerReadinessFramePreservesCoreVoteReady,
   ExactLeaderSchedulerReadinessFramePreservesCoreQCReady,
   ExactLeaderSchedulerReadinessFramePreservesCoreTimeoutReady,
   ExactLeaderSchedulerReadinessFramePreservesCoreTCReady, IsaT(60)
   DEF CoreDeliveryCommandKind

THEOREM ExactLeaderSchedulerReadinessFramePreservesCertifiedCapability ==
  \A item:
    ExactLeaderSchedulerReadinessFrame
      => (CertifiedResponseCapabilityAuthorized(item)'
            <=> CertifiedResponseCapabilityAuthorized(item))
BY IsaT(60)
   DEF ExactLeaderSchedulerReadinessFrame,
       CertifiedResponseCapabilityAuthorized,
       MatchingSentCertifiedRequests,
       FrozenCertifiedResponseBinding,
       FrozenCertifiedRequestRegistration,
       CertifiedResponseAuthenticatedOccurrence, vars

THEOREM ExactLeaderSchedulerReadinessFramePreservesRegularAssembleReady ==
  \A candidate:
    (/\ ExactLeaderSchedulerReadinessFrame
     /\ candidate.kind = "AssembleBody")
      => (RegularCoreCommandReady(candidate)'
            <=> RegularCoreCommandReady(candidate))
BY IsaT(120)
   DEF ExactLeaderSchedulerReadinessFrame,
       RegularCoreCommandReady, CommandMatches,
       AssembleLocalBodyReady, LocalBodyNotSupersededByDecision,
       BodyHeldBy, BodyValidatedBy, NodeIdle, PendingNodes,
       SigningNodes, NodeInstalledTC,
       CurrentVoters, CurrentEpoch, vars

THEOREM ExactLeaderSchedulerReadinessFramePreservesRegularBeginProposalReady ==
  \A candidate:
    (/\ ExactLeaderSchedulerReadinessFrame
     /\ candidate.kind = "BeginProposal")
      => (RegularCoreCommandReady(candidate)'
            <=> RegularCoreCommandReady(candidate))
BY IsaT(120)
   DEF ExactLeaderSchedulerReadinessFrame,
       RegularCoreCommandReady, BeginLocalProposalReady,
       LocalBodyNotSupersededByDecision,
       BodyHeldBy, BodyValidatedBy, NodeIdle, PendingNodes,
       SigningNodes, NodeInstalledTC, LocalProposalFor,
       LocalProposalJustification,
       LocalProposalReproposesJustifiedHigh,
       ProposalWireValidFor, ProposalJustified, SafeToPrepare,
       TCValid, ExactPrepareQcMatchesRef,
       PrepareQcOptionWireValid, QcWireValid,
       PrepareQcRank, PrepareQcSubject,
       CurrentVoters, CurrentEpoch, vars

THEOREM ExactLeaderSchedulerReadinessFramePreservesRegularPersistProposalReady ==
  \A candidate:
    (/\ ExactLeaderSchedulerReadinessFrame
     /\ candidate.kind = "PersistProposal")
      => (RegularCoreCommandReady(candidate)'
            <=> RegularCoreCommandReady(candidate))
BY IsaT(120)
   DEF ExactLeaderSchedulerReadinessFrame,
       RegularCoreCommandReady, CommandMatches,
       PersistProposalReady, ProposalWireValidFor,
       ProposalJustified, SafeToPrepare, TCValid,
       ExactPrepareQcMatchesRef, PrepareQcOptionWireValid,
       QcWireValid, PrepareQcRank, PrepareQcSubject,
       CurrentVoters, CurrentEpoch, vars

THEOREM ExactLeaderSchedulerReadinessFramePreservesRegularLocalReady ==
  \A candidate:
    (/\ ExactLeaderSchedulerReadinessFrame
     /\ candidate.kind
          \in {"AssembleBody", "BeginProposal", "PersistProposal"})
      => (RegularCoreCommandReady(candidate)'
            <=> RegularCoreCommandReady(candidate))
BY ExactLeaderSchedulerReadinessFramePreservesRegularAssembleReady,
   ExactLeaderSchedulerReadinessFramePreservesRegularBeginProposalReady,
   ExactLeaderSchedulerReadinessFramePreservesRegularPersistProposalReady,
   Isa

THEOREM ExactLeaderSchedulerReadinessFramePreservesRegularFetchReady ==
  \A candidate:
    (/\ ExactLeaderSchedulerReadinessFrame
     /\ candidate.kind = "FetchBody")
      => (RegularCoreCommandReady(candidate)'
            <=> RegularCoreCommandReady(candidate))
BY IsaT(120)
   DEF ExactLeaderSchedulerReadinessFrame,
       RegularCoreCommandReady, CommandMatches,
       CertifiedRecoveryFetchFrontier,
       DecisionFetchFrontier, LockedPrepareFetchFrontier,
       DecisionQcValues, DecisionCertifiedBodyRecoveryAuthority,
       LockedPrepareRecoverySource, HistoricalLockedPrepareSource,
       HistoricalLockedPrepareRecoveryProvenance,
       InstalledTcSelectsPrepareFor, ExactLockedCommitIntents,
       NoDecisionForNode, HeldChunksFor, BodyHeldBy,
       SeenProposalValues, FetchBodyReady, vars

THEOREM ExactLeaderSchedulerReadinessFramePreservesRegularRebindReady ==
  \A candidate:
    (/\ ExactLeaderSchedulerReadinessFrame
     /\ candidate.kind = "RebindRetainedBody")
      => (RegularCoreCommandReady(candidate)'
            <=> RegularCoreCommandReady(candidate))
BY IsaT(120)
   DEF ExactLeaderSchedulerReadinessFrame,
       RegularCoreCommandReady, CommandMatches,
       SeenProposalValues, RebindRetainedBodyReady,
       RetainedLockedBodyHeldBy, vars

THEOREM ExactLeaderSchedulerReadinessFramePreservesRegularStoreReady ==
  \A candidate:
    (/\ ExactLeaderSchedulerReadinessFrame
     /\ candidate.kind = "StoreBody")
      => (RegularCoreCommandReady(candidate)'
            <=> RegularCoreCommandReady(candidate))
BY IsaT(120)
   DEF ExactLeaderSchedulerReadinessFrame,
       RegularCoreCommandReady, StoreBodyReady, vars

THEOREM ExactLeaderSchedulerReadinessFramePreservesRegularBodyReady ==
  \A candidate:
    (/\ ExactLeaderSchedulerReadinessFrame
     /\ candidate.kind
          \in {"FetchBody", "RebindRetainedBody", "StoreBody"})
      => (RegularCoreCommandReady(candidate)'
            <=> RegularCoreCommandReady(candidate))
BY ExactLeaderSchedulerReadinessFramePreservesRegularFetchReady,
   ExactLeaderSchedulerReadinessFramePreservesRegularRebindReady,
   ExactLeaderSchedulerReadinessFramePreservesRegularStoreReady, Isa

THEOREM ExactLeaderSchedulerReadinessFramePreservesRegularValidateReady ==
  \A candidate:
    (/\ ExactLeaderSchedulerReadinessFrame
     /\ candidate.kind = "ValidateBody")
      => (RegularCoreCommandReady(candidate)'
            <=> RegularCoreCommandReady(candidate))
BY IsaT(120)
   DEF ExactLeaderSchedulerReadinessFrame,
       RegularCoreCommandReady, CommandMatches,
       SeenProposalValues, DecisionQcValues,
       ValidateBodyReady, RejectBodyReady,
       ValidateDecidedBodyReady, ValidateLockedBodyReady,
       BodyHeldBy, HistoricalLockedPrepareSource,
       HistoricalLockedPrepareRecoveryProvenance,
       InstalledTcSelectsPrepareFor, ExactLockedCommitIntents,
       NoDecisionForNode, CurrentVoters, CurrentEpoch, vars

THEOREM ExactLeaderSchedulerReadinessFramePreservesProposalWireValidity ==
  \A node, proposal:
    ExactLeaderSchedulerReadinessFrame
      => (ProposalWireValidFor(node, proposal)'
            <=> ProposalWireValidFor(node, proposal))
BY IsaT(120)
   DEF ExactLeaderSchedulerReadinessFrame,
       ProposalWireValidFor, ProposalJustified, SafeToPrepare,
       TCValid, ExactPrepareQcMatchesRef,
       PrepareQcOptionWireValid, QcWireValid,
       PrepareQcRank, PrepareQcSubject,
       CurrentVoters, CurrentEpoch, vars

THEOREM ExactLeaderSchedulerReadinessFramePreservesBeginPrepareReady ==
  \A node, proposal:
    ExactLeaderSchedulerReadinessFrame
      => (BeginPrepareReady(node, proposal)'
            <=> BeginPrepareReady(node, proposal))
BY ExactLeaderSchedulerReadinessFramePreservesProposalWireValidity,
   IsaT(120)
   DEF ExactLeaderSchedulerReadinessFrame,
       BeginPrepareReady, PrepareSignerAvailability,
       BodyHeldBy, BodyValidatedBy,
       NodeIdle, PendingNodes, SigningNodes, NodeTimedOut,
       CurrentVoters, CurrentEpoch, vars

THEOREM ExactLeaderSchedulerReadinessFramePreservesRegularBeginPrepareReady ==
  \A candidate:
    (/\ ExactLeaderSchedulerReadinessFrame
     /\ candidate.kind = "BeginPrepare")
      => (RegularCoreCommandReady(candidate)'
            <=> RegularCoreCommandReady(candidate))
BY ExactLeaderSchedulerReadinessFramePreservesBeginPrepareReady,
   IsaT(120)
   DEF ExactLeaderSchedulerReadinessFrame,
       RegularCoreCommandReady, CommandMatches,
       SeenProposalValues, vars

THEOREM ExactLeaderSchedulerReadinessFramePreservesRegularPersistPrepareReady ==
  \A candidate:
    (/\ ExactLeaderSchedulerReadinessFrame
     /\ candidate.kind = "PersistPrepare")
      => (RegularCoreCommandReady(candidate)'
            <=> RegularCoreCommandReady(candidate))
BY IsaT(180)
   DEF ExactLeaderSchedulerReadinessFrame,
       RegularCoreCommandReady, CommandMatches,
       SeenProposalValues, ReceivedQcValues,
       BeginPrepareReady, PersistPrepareReady,
       BeginObservePrepareReady, PersistObservePrepareReady,
       NodeIdle, PendingNodes, SigningNodes,
       ProposalWireValidFor, ProposalJustified, SafeToPrepare,
       TCValid, ExactPrepareQcMatchesRef,
       PrepareQcOptionWireValid, QcWireValid,
       PrepareQcRank, PrepareQcSubject,
       PrepareSignerAvailability, BodyHeldBy, BodyValidatedBy,
       NodeTimedOut, CurrentVoters, CurrentEpoch, vars

THEOREM ExactLeaderSchedulerReadinessFramePreservesRegularBeginObservePrepareReady ==
  \A candidate:
    (/\ ExactLeaderSchedulerReadinessFrame
     /\ candidate.kind = "BeginObservePrepare")
      => (RegularCoreCommandReady(candidate)'
            <=> RegularCoreCommandReady(candidate))
BY IsaT(180)
   DEF ExactLeaderSchedulerReadinessFrame,
       RegularCoreCommandReady, CommandMatches,
       SeenProposalValues, ReceivedQcValues,
       BeginPrepareReady, PersistPrepareReady,
       BeginObservePrepareReady, PersistObservePrepareReady,
       NodeIdle, PendingNodes, SigningNodes,
       ProposalWireValidFor, ProposalJustified, SafeToPrepare,
       TCValid, ExactPrepareQcMatchesRef,
       PrepareQcOptionWireValid, QcWireValid,
       PrepareQcRank, PrepareQcSubject,
       PrepareSignerAvailability, BodyHeldBy, BodyValidatedBy,
       NodeTimedOut, CurrentVoters, CurrentEpoch, vars

THEOREM ExactLeaderSchedulerReadinessFramePreservesRegularPersistObservePrepareReady ==
  \A candidate:
    (/\ ExactLeaderSchedulerReadinessFrame
     /\ candidate.kind = "PersistObservePrepare")
      => (RegularCoreCommandReady(candidate)'
            <=> RegularCoreCommandReady(candidate))
BY IsaT(180)
   DEF ExactLeaderSchedulerReadinessFrame,
       RegularCoreCommandReady, CommandMatches,
       SeenProposalValues, ReceivedQcValues,
       BeginPrepareReady, PersistPrepareReady,
       BeginObservePrepareReady, PersistObservePrepareReady,
       NodeIdle, PendingNodes, SigningNodes,
       ProposalWireValidFor, ProposalJustified, SafeToPrepare,
       TCValid, ExactPrepareQcMatchesRef,
       PrepareQcOptionWireValid, QcWireValid,
       PrepareQcRank, PrepareQcSubject,
       PrepareSignerAvailability, BodyHeldBy, BodyValidatedBy,
       NodeTimedOut, CurrentVoters, CurrentEpoch, vars

THEOREM ExactLeaderSchedulerReadinessFramePreservesRegularPrepareReady ==
  \A candidate:
    (/\ ExactLeaderSchedulerReadinessFrame
     /\ candidate.kind
          \in {"BeginPrepare", "PersistPrepare",
               "BeginObservePrepare", "PersistObservePrepare"})
      => (RegularCoreCommandReady(candidate)'
            <=> RegularCoreCommandReady(candidate))
BY ExactLeaderSchedulerReadinessFramePreservesRegularBeginPrepareReady,
   ExactLeaderSchedulerReadinessFramePreservesRegularPersistPrepareReady,
   ExactLeaderSchedulerReadinessFramePreservesRegularBeginObservePrepareReady,
   ExactLeaderSchedulerReadinessFramePreservesRegularPersistObservePrepareReady,
   Isa

THEOREM ExactLeaderSchedulerReadinessFramePreservesRegularBeginLockCommitReady ==
  \A candidate:
    (/\ ExactLeaderSchedulerReadinessFrame
     /\ candidate.kind = "BeginLockCommit")
      => (RegularCoreCommandReady(candidate)'
            <=> RegularCoreCommandReady(candidate))
BY ExactLeaderSchedulerReadinessFramePreservesNetworkItems, IsaT(180)
   DEF ExactLeaderSchedulerReadinessFrame,
       RegularCoreCommandReady, CommandMatches,
       LockCommitQcValues, ReceivedQcValues,
       BeginLockCommandEvidenceMatches, BeginLockCommitReady,
       PersistLockCommitReady, FormCommitQCReady,
       BeginDecisionReady, CurrentOpenPrepareForCommit,
       NodeTimedOut, BodyHeldBy, BodyValidatedBy,
       RetainedLockedBodyRecord, VoteSignersAt,
       CommitRoundAdmissible, LockedPrepareRound,
       QcWireValid, NodeIdle, PendingNodes, SigningNodes,
       CurrentVoters, CurrentEpoch, vars

THEOREM ExactLeaderSchedulerReadinessFramePreservesRegularPersistLockCommitReady ==
  \A candidate:
    (/\ ExactLeaderSchedulerReadinessFrame
     /\ candidate.kind = "PersistLockCommit")
      => (RegularCoreCommandReady(candidate)'
            <=> RegularCoreCommandReady(candidate))
BY ExactLeaderSchedulerReadinessFramePreservesNetworkItems, IsaT(180)
   DEF ExactLeaderSchedulerReadinessFrame,
       RegularCoreCommandReady, CommandMatches,
       LockCommitQcValues, ReceivedQcValues,
       BeginLockCommandEvidenceMatches, BeginLockCommitReady,
       PersistLockCommitReady, FormCommitQCReady,
       BeginDecisionReady, CurrentOpenPrepareForCommit,
       NodeTimedOut, BodyHeldBy, BodyValidatedBy,
       RetainedLockedBodyRecord, VoteSignersAt,
       CommitRoundAdmissible, LockedPrepareRound,
       QcWireValid, NodeIdle, PendingNodes, SigningNodes,
       CurrentVoters, CurrentEpoch, vars

THEOREM ExactLeaderSchedulerReadinessFramePreservesRegularFormCommitQCReady ==
  \A candidate:
    (/\ ExactLeaderSchedulerReadinessFrame
     /\ candidate.kind = "FormCommitQC")
      => (RegularCoreCommandReady(candidate)'
            <=> RegularCoreCommandReady(candidate))
BY ExactLeaderSchedulerReadinessFramePreservesNetworkItems, IsaT(180)
   DEF ExactLeaderSchedulerReadinessFrame,
       RegularCoreCommandReady, CommandMatches,
       LockCommitQcValues, ReceivedQcValues,
       BeginLockCommandEvidenceMatches, BeginLockCommitReady,
       PersistLockCommitReady, FormCommitQCReady,
       BeginDecisionReady, CurrentOpenPrepareForCommit,
       NodeTimedOut, BodyHeldBy, BodyValidatedBy,
       RetainedLockedBodyRecord, VoteSignersAt,
       CommitRoundAdmissible, LockedPrepareRound,
       QcWireValid, NodeIdle, PendingNodes, SigningNodes,
       CurrentVoters, CurrentEpoch, vars

THEOREM ExactLeaderSchedulerReadinessFramePreservesRegularBeginDecisionReady ==
  \A candidate:
    (/\ ExactLeaderSchedulerReadinessFrame
     /\ candidate.kind = "BeginDecision")
      => (RegularCoreCommandReady(candidate)'
            <=> RegularCoreCommandReady(candidate))
BY ExactLeaderSchedulerReadinessFramePreservesNetworkItems, IsaT(180)
   DEF ExactLeaderSchedulerReadinessFrame,
       RegularCoreCommandReady, CommandMatches,
       LockCommitQcValues, ReceivedQcValues,
       BeginLockCommandEvidenceMatches, BeginLockCommitReady,
       PersistLockCommitReady, FormCommitQCReady,
       BeginDecisionReady, CurrentOpenPrepareForCommit,
       NodeTimedOut, BodyHeldBy, BodyValidatedBy,
       RetainedLockedBodyRecord, VoteSignersAt,
       CommitRoundAdmissible, LockedPrepareRound,
       QcWireValid, NodeIdle, PendingNodes, SigningNodes,
       CurrentVoters, CurrentEpoch, vars

THEOREM ExactLeaderSchedulerReadinessFramePreservesRegularCommitReady ==
  \A candidate:
    (/\ ExactLeaderSchedulerReadinessFrame
     /\ candidate.kind
          \in {"BeginLockCommit", "PersistLockCommit",
               "FormCommitQC", "BeginDecision"})
      => (RegularCoreCommandReady(candidate)'
            <=> RegularCoreCommandReady(candidate))
BY ExactLeaderSchedulerReadinessFramePreservesRegularBeginLockCommitReady,
   ExactLeaderSchedulerReadinessFramePreservesRegularPersistLockCommitReady,
   ExactLeaderSchedulerReadinessFramePreservesRegularFormCommitQCReady,
   ExactLeaderSchedulerReadinessFramePreservesRegularBeginDecisionReady,
   Isa

THEOREM ExactLeaderSchedulerReadinessFramePreservesRegularPersistTimeoutReady ==
  \A candidate:
    (/\ ExactLeaderSchedulerReadinessFrame
     /\ candidate.kind = "PersistTimeout")
      => (RegularCoreCommandReady(candidate)'
            <=> RegularCoreCommandReady(candidate))
BY ExactLeaderSchedulerReadinessFramePreservesNetworkItems,
   ExactLeaderSchedulerReadinessFramePreservesCertifiedCapability,
   IsaT(180)
   DEF ExactLeaderSchedulerReadinessFrame,
       RegularCoreCommandReady, CommandMatches,
       PersistTimeoutReady, ReceivedTcValues,
       InstallTcEvidenceMatches, BeginInstallTCReady,
       StrictSameRoundTcUpgrade, NodeInstalledTC, TcHighRank,
       NodeIdle, PendingNodes, SigningNodes, NoDecisionForNode,
       CertifiedResponseCapabilityAuthorized,
       InstallCertifiedBodyEffectReady, BodyHeldBy, vars

THEOREM ExactLeaderSchedulerReadinessFramePreservesRegularBeginInstallTCReady ==
  \A candidate:
    (/\ ExactLeaderSchedulerReadinessFrame
     /\ candidate.kind = "BeginInstallTC")
      => (RegularCoreCommandReady(candidate)'
            <=> RegularCoreCommandReady(candidate))
BY ExactLeaderSchedulerReadinessFramePreservesNetworkItems,
   ExactLeaderSchedulerReadinessFramePreservesCertifiedCapability,
   IsaT(180)
   DEF ExactLeaderSchedulerReadinessFrame,
       RegularCoreCommandReady, CommandMatches,
       PersistTimeoutReady, ReceivedTcValues,
       InstallTcEvidenceMatches, BeginInstallTCReady,
       StrictSameRoundTcUpgrade, NodeInstalledTC, TcHighRank,
       NodeIdle, PendingNodes, SigningNodes, NoDecisionForNode,
       CertifiedResponseCapabilityAuthorized,
       InstallCertifiedBodyEffectReady, BodyHeldBy, vars

THEOREM ExactLeaderSchedulerReadinessFramePreservesRegularFetchCertifiedBodyReady ==
  \A candidate:
    (/\ ExactLeaderSchedulerReadinessFrame
     /\ candidate.kind = "FetchCertifiedBody")
      => (RegularCoreCommandReady(candidate)'
            <=> RegularCoreCommandReady(candidate))
BY ExactLeaderSchedulerReadinessFramePreservesNetworkItems,
   ExactLeaderSchedulerReadinessFramePreservesCertifiedCapability,
   IsaT(180)
   DEF ExactLeaderSchedulerReadinessFrame,
       RegularCoreCommandReady, CommandMatches,
       PersistTimeoutReady, ReceivedTcValues,
       InstallTcEvidenceMatches, BeginInstallTCReady,
       StrictSameRoundTcUpgrade, NodeInstalledTC, TcHighRank,
       NodeIdle, PendingNodes, SigningNodes, NoDecisionForNode,
       CertifiedResponseCapabilityAuthorized,
       InstallCertifiedBodyEffectReady, BodyHeldBy, vars

THEOREM ExactLeaderSchedulerReadinessFramePreservesRegularTimeoutReady ==
  \A candidate:
    (/\ ExactLeaderSchedulerReadinessFrame
     /\ candidate.kind
          \in {"PersistTimeout", "BeginInstallTC",
               "FetchCertifiedBody"})
      => (RegularCoreCommandReady(candidate)'
            <=> RegularCoreCommandReady(candidate))
BY ExactLeaderSchedulerReadinessFramePreservesRegularPersistTimeoutReady,
   ExactLeaderSchedulerReadinessFramePreservesRegularBeginInstallTCReady,
   ExactLeaderSchedulerReadinessFramePreservesRegularFetchCertifiedBodyReady,
   Isa

THEOREM ExactLeaderSchedulerReadinessFramePreservesRegularReady ==
  \A candidate:
    ExactLeaderSchedulerReadinessFrame
      => (ExecuteRegularCommandReady(candidate)'
            <=> ExecuteRegularCommandReady(candidate))
BY ExactLeaderSchedulerReadinessFramePreservesRegularLocalReady,
   ExactLeaderSchedulerReadinessFramePreservesRegularBodyReady,
   ExactLeaderSchedulerReadinessFramePreservesRegularValidateReady,
   ExactLeaderSchedulerReadinessFramePreservesRegularPrepareReady,
   ExactLeaderSchedulerReadinessFramePreservesRegularCommitReady,
   ExactLeaderSchedulerReadinessFramePreservesRegularTimeoutReady, IsaT(60)
   DEF ExecuteRegularCommandReady, RegularCoreCommandReady

THEOREM ExactLeaderSchedulerReadinessFramePreservesChunkDeliveryReady ==
  \A candidate:
    ExactLeaderSchedulerReadinessFrame
      => (ExecuteChunkDeliveryReady(candidate)'
            <=> ExecuteChunkDeliveryReady(candidate))
BY Isa
   DEF ExactLeaderSchedulerReadinessFrame,
       ExecuteChunkDeliveryReady, vars

THEOREM ExactLeaderSchedulerReadinessFramePreservesRejectJunkReady ==
  \A candidate:
    ExactLeaderSchedulerReadinessFrame
      => (ExecuteRejectAuthenticatedJunkReady(candidate)'
            <=> ExecuteRejectAuthenticatedJunkReady(candidate))
BY Isa
   DEF ExactLeaderSchedulerReadinessFrame,
       ExecuteRejectAuthenticatedJunkReady, vars

THEOREM ExactLeaderSchedulerReadinessFramePreservesExecutionReady ==
  \A candidate:
    ExactLeaderSchedulerReadinessFrame
      => (CommandExecutionReady(candidate)'
            <=> CommandExecutionReady(candidate))
BY IsaT(120)
   DEF ExactLeaderSchedulerReadinessFrame,
       CommandExecutionReady, vars

THEOREM ExactLeaderSchedulerReadinessFramePreservesDispatchable ==
  \A candidate:
    ExactLeaderSchedulerReadinessFrame
      => (CommandDispatchable(candidate)'
            <=> CommandDispatchable(candidate))
BY ExactLeaderSchedulerReadinessFramePreservesExecutionReady,
   IsaT(120)
   DEF ExactLeaderSchedulerReadinessFrame,
       CommandDispatchable, CandidateConsumerCurrent,
       LocalAssemblyBusyDispatchAllowed, NodeIdle, vars

THEOREM ExactLeaderSchedulerReadinessFramePreservesAlternateOwner ==
  \A candidate:
    ExactLeaderSchedulerReadinessFrame
      => (DispatchableSameIdentityLeaderOwner(candidate)'
            <=> DispatchableSameIdentityLeaderOwner(candidate))
BY ExactLeaderSchedulerReadinessFramePreservesRank,
   ExactLeaderSchedulerReadinessFramePreservesDispatchable,
   IsaT(120)
   DEF ExactLeaderSchedulerReadinessFrame,
       DispatchableSameIdentityLeaderOwner

THEOREM ExactLeaderSchedulerReadinessFramePreservesEvidence ==
  \A candidate, rank:
    ExactLeaderSchedulerReadinessFrame
      => (ExactLeaderEvidenceAt(candidate, rank)'
            <=> ExactLeaderEvidenceAt(candidate, rank))
BY IsaT(120)
   DEF ExactLeaderSchedulerReadinessFrame,
       ExactLeaderEvidenceAt, ProposalEvidenceAt,
       PrepareEvidenceAt, CommitEvidenceAt,
       ViewChangeEvidenceAt, NodeIdle, vars

THEOREM ExactLeaderSchedulerReadinessFramePreservesDiscardProvenance ==
  \A candidate, rank:
    ExactLeaderSchedulerReadinessFrame
      => (ExactLeaderDiscardProvenanceAt(candidate, rank)'
            <=> ExactLeaderDiscardProvenanceAt(candidate, rank))
BY ExactLeaderSchedulerReadinessFramePreservesEvidence,
   IsaT(120)
   DEF ExactLeaderSchedulerReadinessFrame,
       ExactLeaderDiscardProvenanceAt,
       AuthenticatedLeaderDiscardProvenance,
       PriorPhaseLeaderDiscardProvenance,
       IngressItemHasAuthenticatedHistory,
       CertifiedResponseAuthenticatedOccurrence,
       ProposalEvidenceAt, PrepareEvidenceAt, CommitEvidenceAt,
       vars

THEOREM ExactLeaderSchedulerReadinessFramePreservesParking ==
  \A candidate:
    ExactLeaderSchedulerReadinessFrame
      => (ExactLeaderSchedulerParked(candidate)'
            <=> ExactLeaderSchedulerParked(candidate))
BY Isa
   DEF ExactLeaderSchedulerReadinessFrame,
       ExactLeaderSchedulerParked, vars

THEOREM ExactLeaderSchedulerReadinessFramePreservesIdleness ==
  \A node:
    ExactLeaderSchedulerReadinessFrame
      => (NodeIdle(node)' <=> NodeIdle(node))
BY Isa
   DEF ExactLeaderSchedulerReadinessFrame, NodeIdle, vars

THEOREM ExactLeaderSchedulerReadinessFramePreservesInvariant ==
  /\ ExactLeaderSchedulerOriginReadinessInvariant
  /\ ExactLeaderSchedulerReadinessFrame
  => ExactLeaderSchedulerOriginReadinessInvariant'
BY ExactLeaderSchedulerReadinessFramePreservesRank,
   ExactLeaderSchedulerReadinessFramePreservesExecutionReady,
   ExactLeaderSchedulerReadinessFramePreservesDispatchable,
   ExactLeaderSchedulerReadinessFramePreservesAlternateOwner,
   ExactLeaderSchedulerReadinessFramePreservesEvidence,
   ExactLeaderSchedulerReadinessFramePreservesDiscardProvenance,
   ExactLeaderSchedulerReadinessFramePreservesParking,
   ExactLeaderSchedulerReadinessFramePreservesIdleness,
   IsaT(120)
   DEF ExactLeaderSchedulerOriginReadinessInvariant,
       ExactLeaderSchedulerOriginProvenanceInvariant,
       ExactLeaderSchedulerIdleReadinessInvariant

THEOREM AsyncTickPreservesExactLeaderSchedulerOriginReadiness ==
  /\ ExactLeaderSchedulerOriginReadinessInvariant
  /\ AsyncTick
  => ExactLeaderSchedulerOriginReadinessInvariant'
BY ExactLeaderSchedulerReadinessFramePreservesInvariant, Isa
   DEF ExactLeaderSchedulerReadinessFrame,
       AsyncTick, AsyncNonClockVars

THEOREM AsyncNetworkStepPreservesExactLeaderSchedulerOriginReadiness ==
  /\ ExactLeaderSchedulerOriginReadinessInvariant
  /\ AsyncNetworkStep
  => ExactLeaderSchedulerOriginReadinessInvariant'
BY ExactLeaderSchedulerReadinessFramePreservesInvariant, Isa
   DEF ExactLeaderSchedulerReadinessFrame,
       AsyncNetworkStep, AdmitIngressPacket,
       AdmitHiddenPacket, CoalesceHiddenPacket,
       DropPolicyRejectedHiddenPacket,
       AsyncIoVars, AsyncDeferredVars, LeaveCausalQueues

THEOREM SameConsumerLeaderDiscardIsIdleAndDisabled ==
  \A candidate:
    /\ AsyncStrongTypeInvariant
    /\ SameConsumerLeaderDiscard(candidate)
      => /\ NodeIdle(candidate.node)
         /\ ~CommandDispatchable(candidate)
BY Isa
   DEF AsyncStrongTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
       AsyncDeferredTypeInvariant, AsyncDeferredContentTypeInvariant,
       AsyncCommandQueueOwnership,
       SameConsumerLeaderDiscard, DeferredWorkServiceable,
       FifoRuntimeStep, DeferredDrainStep, NextNodeCommand,
       NextDeferredCommand, DeferredClassQueue, SequenceSet

THEOREM ExactLeaderEvidenceSurvivesSameConsumerDiscard ==
  \A candidate, rank:
    /\ ExactLeaderEvidenceAt(candidate, rank)
    /\ SameConsumerLeaderDiscard(candidate)
    => ExactLeaderEvidenceAt(candidate, rank)'
BY Isa
   DEF SameConsumerLeaderDiscard, ExactLeaderEvidenceAt,
       ProposalEvidenceAt, PrepareEvidenceAt, CommitEvidenceAt,
       ViewChangeEvidenceAt, DiscardCommand

THEOREM ExactLeaderDiscardProvenanceSurvivesSameConsumerDiscard ==
  \A candidate, rank:
    /\ ExactLeaderDiscardProvenanceAt(candidate, rank)
    /\ SameConsumerLeaderDiscard(candidate)
    => ExactLeaderDiscardProvenanceAt(candidate, rank)'
BY Isa
   DEF ExactLeaderDiscardProvenanceAt,
       AuthenticatedLeaderDiscardProvenance,
       PriorPhaseLeaderDiscardProvenance,
       IngressItemHasAuthenticatedHistory,
       CertifiedResponseAuthenticatedOccurrence,
       ProposalEvidenceAt, PrepareEvidenceAt, CommitEvidenceAt,
       SameConsumerLeaderDiscard, FifoRuntimeStep,
       DeferredDrainStep, DiscardCommand

THEOREM DispatchableSameIdentityOwnerIsSemanticOwner ==
  \A candidate:
    DispatchableSameIdentityLeaderOwner(candidate)
      => SameIdentityLeaderOwner(candidate)
BY DEF DispatchableSameIdentityLeaderOwner,
       SameIdentityLeaderOwner

THEOREM OtherExactLeaderOwnerSurvivesSameConsumerDiscard ==
  \A candidate:
    /\ AsyncProgressOwnershipInvariant
    /\ SameIdentityLeaderOwner(candidate)
    /\ SameConsumerLeaderDiscard(candidate)
    => SameIdentityLeaderOwner(candidate)'
BY SequenceWithoutIndexRetainsOtherValue,
   TailRetainsNonHeadValue, IsaT(300)
   DEF SameIdentityLeaderOwner, SameConsumerLeaderDiscard,
       ExactLeaderCandidateRank,
       ResponsiveProtectedCandidateOwned, ProtectedCandidateOwned,
       ProtectedServiceCandidate, CandidateScheduled,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates,
       AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       FifoRuntimeStep, DeferredDrainStep,
       RemoveNextNodeCommand, RemoveNextDeferredCommand,
       DeferredClassQueue, DiscardCommand, LeaveCausalQueues,
       SequenceSet

THEOREM SchedulerOriginReadinessExcludesUnexplainedDiscard ==
  \A candidate \in AsyncCandidateSet,
     rank \in (1..5) \X Nat:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ ExactLeaderSchedulerOriginReadinessInvariant
    /\ ExactLeaderCandidateRank(candidate, rank)
    /\ SameConsumerLeaderDiscard(candidate)
    => ~UnexplainedSameConsumerLeaderDiscard(candidate, rank)
BY SameConsumerLeaderDiscardIsIdleAndDisabled,
   ExactLeaderEvidenceSurvivesSameConsumerDiscard,
   ExactLeaderDiscardProvenanceSurvivesSameConsumerDiscard,
   DispatchableSameIdentityOwnerIsSemanticOwner,
   OtherExactLeaderOwnerSurvivesSameConsumerDiscard, Isa
   DEF ExactLeaderSchedulerOriginReadinessInvariant,
       ExactLeaderSchedulerIdleReadinessInvariant,
       UnexplainedSameConsumerLeaderDiscard

UnexplainedExactLeaderExitAction ==
  \E candidate \in AsyncCandidateSet,
     rank \in (1..5) \X Nat:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ ExactLeaderOwnerExitStep(candidate, rank)
    /\ UnexplainedSameConsumerLeaderDiscard(candidate, rank)

THEOREM SchedulerOriginReadinessExcludesUnexplainedExitAction ==
  ExactLeaderSchedulerOriginReadinessInvariant
    => [~UnexplainedExactLeaderExitAction]_AsyncAllVars
BY SchedulerOriginReadinessExcludesUnexplainedDiscard, Isa
   DEF UnexplainedExactLeaderExitAction, ExactLeaderOwnerExitStep

NoUnexplainedSameConsumerLeaderDiscardProperty(specification) ==
  specification
    => [][~UnexplainedExactLeaderExitAction]_AsyncAllVars

ExactLeaderExitSafetyKernelProperty(specification) ==
  NoUnexplainedSameConsumerLeaderDiscardProperty(specification)

THEOREM SchedulerOriginReadinessReducesToExactLeaderExitSafety ==
  \A initialContext:
    ExactLeaderSchedulerOriginReadinessProperty(
      AsyncLiveSpecAt(initialContext))
      => ExactLeaderExitSafetyKernelProperty(
           AsyncLiveSpecAt(initialContext))
BY SchedulerOriginReadinessExcludesUnexplainedExitAction, PTL
   DEF ExactLeaderSchedulerOriginReadinessProperty,
       ExactLeaderExitSafetyKernelProperty,
       NoUnexplainedSameConsumerLeaderDiscardProperty

(***************************************************************************
Given the explicit scheduler-origin/readiness kernel, the already-proved
starvation theorem yields a semantic handoff for each fixed candidate.
***************************************************************************)

ExactLeaderCandidateExitOutcome(candidate, rank) ==
  \/ NodeHasDecision(candidate.node)
  \/ NodeHasApplication(candidate.node)
  \/ ~CandidateConsumerCurrent(candidate)
  \/ ExactLeaderCandidatePostMilestone(candidate, rank)
  \/ SameIdentityLeaderOwner(candidate)
  \/ ExactLeaderEvidenceAt(candidate, rank)
  \/ ExactLeaderDiscardProvenanceAt(candidate, rank)

ExactLeaderCandidateSemanticHandoffProperty(specification) ==
  specification
    => \A candidate \in AsyncCandidateSet,
          rank \in (1..5) \X Nat:
         (gst /\ ExactLeaderCandidateRank(candidate, rank))
           ~> ExactLeaderCandidateExitOutcome(candidate, rank)

THEOREM ExactDiscardSafetyClosesAdmittedCandidateHandoffs ==
  \A initialContext:
    ExactLeaderExitSafetyKernelProperty(
      AsyncLiveSpecAt(initialContext))
      => ExactLeaderCandidateSemanticHandoffProperty(
           AsyncLiveSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext,
                ExactLeaderExitSafetyKernelProperty(
                  AsyncLiveSpecAt(initialContext))
         PROVE ExactLeaderCandidateSemanticHandoffProperty(
                 AsyncLiveSpecAt(initialContext))
    <2>1. ExactLeaderRankedCandidateExitProperty(
             AsyncLiveSpecAt(initialContext))
      BY AsyncLiveExactLeaderRankedCandidatesExit
    <2>2. AsyncLiveSpecAt(initialContext)
            => []AsyncStrongTypeInvariant
      BY AsyncSpecAlwaysStrongTypeInvariant,
         AsyncLiveSpecProjectsAsyncSpec
    <2>3. ASSUME AsyncLiveSpecAt(initialContext)
           PROVE \A candidate \in AsyncCandidateSet,
                     rank \in (1..5) \X Nat:
              (gst /\ ExactLeaderCandidateRank(candidate, rank))
                ~> ExactLeaderCandidateExitOutcome(candidate, rank)
      <3>1. ASSUME NEW candidate \in AsyncCandidateSet,
                    NEW rank \in (1..5) \X Nat
             PROVE (gst /\ ExactLeaderCandidateRank(candidate, rank))
                     ~> ExactLeaderCandidateExitOutcome(candidate, rank)
        <4>1. (gst /\ ExactLeaderCandidateRank(candidate, rank))
                 ~> ~ResponsiveProtectedCandidateOwned(candidate)
          BY <2>1, <2>3
             DEF ExactLeaderRankedCandidateExitProperty
        <4>2. [][(/\ gst
                    /\ ExactLeaderCandidateRank(candidate, rank)
                    /\ AsyncNext
                    /\ ~ResponsiveProtectedCandidateOwned(candidate)'
                   => ExactLeaderCandidateExitOutcome(candidate, rank)')]_AsyncAllVars
          BY <1>1, <2>2, <2>3,
             RankedLeaderOwnerExitDecomposition, PTL
             DEF ExactLeaderExitSafetyKernelProperty,
                 NoUnexplainedSameConsumerLeaderDiscardProperty,
                 ExactLeaderOwnerExitStep,
                 ExactLeaderCandidateExitOutcome,
                 TerminalLeaderExit,
                 PacemakerRetiredLeaderOwner,
                 ExecutionProducedLeaderMilestone,
                 CoveredSameConsumerLeaderDiscard
        <4> QED BY <4>1, <4>2, PTL
      <3> QED BY <3>1
    <2> QED BY <2>3
         DEF ExactLeaderCandidateSemanticHandoffProperty
  <1> QED BY <1>1

(***************************************************************************
Packet-to-candidate ownership.

Wire occurrences are not in `CandidateServiceRank`.  The following predicates
split the exact residual without depending on the separate reset-boundary
leaf:

  * same-lane head-of-line shadow;
  * a due lane head rejected by capacity/ownership admission gates; and
  * an admitted item whose runner is still at Runtime/Local rather than the
    Ingress drain which creates either `DeliveryCandidate(item)` or the
    specialized `CertifiedResponseCandidate(item)`.

The explicit Runtime value is important.  Existing `RuntimeReachRank` is zero
at Runtime and resets upward on Runtime->Local, so it cannot by itself prove
reachability of the next Ingress drain for a newly admitted wire item.
***************************************************************************)

LeaderCertifiedResponseRelevant(item) ==
  /\ item.kind = "CertifiedResponse"
  /\ CertifiedResponseAuthenticatedOccurrence(item)
  /\ \/ CertifiedResponseAuthorized(item)
     \/ CandidateScheduled(CertifiedResponseCandidate(item))
     \/ SameIdentityLeaderOwner(CertifiedResponseCandidate(item))
     \/ BodyHeldBy(durableBodies, item.envelope.recipient, context,
                   item.envelope.view, item.envelope.subject)
     \/ nodeView[item.envelope.recipient] > item.envelope.view
     \/ NodeHasDecision(item.envelope.recipient)

LeaderWireKinds ==
  {"Proposal", "Chunk", "PrepareVote", "PrepareQC",
   "CommitVote", "CommitQC", "TimeoutVote", "TimeoutCertificate",
   "CertifiedResponse"}

LeaderWireItem(item) ==
  /\ item \in AsyncNetworkItems
  /\ item.kind \in LeaderWireKinds
  /\ item.envelope.recipient \in AsyncCurrentResponsiveVoters
  /\ IF item.kind = "CertifiedResponse"
     THEN LeaderCertifiedResponseRelevant(item)
     ELSE TRUE

LeaderWireCarriesContext(item, leaderContext) ==
  /\ DeliveryHeight(item) = leaderContext.height
  /\ CASE item.kind = "Proposal" ->
            item.envelope.proposal.context = leaderContext
       [] item.kind
            \in {"PrepareVote", "CommitVote", "TimeoutVote"} ->
            item.envelope.vote.context = leaderContext
       [] item.kind \in {"PrepareQC", "CommitQC"} ->
            item.envelope.qc.context = leaderContext
       [] item.kind = "TimeoutCertificate" ->
            item.envelope.tc.context = leaderContext
       [] OTHER -> item.envelope.height = leaderContext.height

LeaderWireExactSemanticIdentity(
    item, leaderContext, witness, roundView, subject) ==
  /\ LeaderWireItem(item)
  /\ item \in asyncSentItems
  /\ leaderContext \in ContextRecords
  /\ witness \in Responsive \cap VotingRoster(leaderContext.epoch)
  /\ roundView \in Views
  /\ subject \in SubjectOrNone
  /\ item.envelope.recipient = witness
  /\ DeliveryView(item) = roundView
  /\ DeliverySubject(item) = subject
  /\ LeaderWireCarriesContext(item, leaderContext)

LeaderWireCurrentContextWitnessIdentity(item) ==
  LeaderWireExactSemanticIdentity(
    item, context, item.envelope.recipient,
    DeliveryView(item), DeliverySubject(item))

\* Generic unauthenticated transport-completion traffic is drainable cleanup,
\* not a semantic leader corridor.  This classifier depends only on immutable
\* item fields; unlike `IngressPacketPolicyRejected`, it cannot change while a
\* frozen packet waits.
LeaderWireProductiveTransportIdentity(item) ==
  /\ LeaderWireCurrentContextWitnessIdentity(item)
  /\ ~(/\ IngressAdmissionClass(item) = "TransportCompletion"
       /\ IngressResourceSource(item) = AsyncUntrustedSource)

LeaderWireIngressOwned(item) ==
  /\ IngressResourceSource(item) \in AsyncIngressSources
  /\ item \in SequenceSet(
       IngressLane(item.envelope.recipient,
                   IngressResourceSource(item)))

LeaderWireCandidateOwned(item) ==
  IF item.kind = "CertifiedResponse"
  THEN CandidateScheduled(CertifiedResponseCandidate(item))
  ELSE CandidateScheduled(DeliveryCandidate(item))

LeaderWireConsumerMilestone(item) ==
  \/ NodeHasDecision(item.envelope.recipient)
  \/ NodeHasApplication(item.envelope.recipient)
  \/ CASE item.kind = "Proposal" ->
            ProposalAt(item.envelope.recipient,
                       item.envelope.proposal) \in seenProposals
       [] item.kind = "Chunk" ->
            AsyncChunkReceipt(item.envelope.recipient,
                              item.envelope.view,
                              item.envelope.subject,
                              item.envelope.chunk) \in asyncHeldChunks
       [] item.kind \in {"PrepareVote", "CommitVote"} ->
            VoteAt(item.envelope.recipient,
                   item.envelope.vote) \in receivedVotes
       [] item.kind \in {"PrepareQC", "CommitQC"} ->
            QcAt(item.envelope.recipient,
                 item.envelope.qc) \in receivedQCs
       [] item.kind = "TimeoutVote" ->
            ReceivedTimeoutVoteAt(
              item.envelope.recipient,
              item.envelope.vote.signer,
              item.envelope.vote.view)
       [] item.kind = "TimeoutCertificate" ->
            TcAt(item.envelope.recipient,
                 item.envelope.tc) \in receivedTCs
              \/ nodeView[item.envelope.recipient]
                   > item.envelope.tc.view
       [] item.kind = "CertifiedResponse" ->
            \/ SameIdentityLeaderOwner(
                 CertifiedResponseCandidate(item))
            \/ BodyHeldBy(durableBodies,
                          item.envelope.recipient, context,
                          item.envelope.view, item.envelope.subject)
            \/ nodeView[item.envelope.recipient] > item.envelope.view
       [] OTHER -> FALSE

LeaderWirePacket(packet) ==
  /\ packet \in AsyncPacketSet
  /\ LeaderWireItem(packet.item)

LeaderWireIngressRemovalGate(item) ==
  \/ /\ ~IngressHasCoalescingOwner(item)
        /\ CanAdmitIngressItem(item)
  \/ /\ IngressHasCoalescingOwner(item)
        /\ \/ item.kind # "CertifiedResponse"
           \/ CertifiedResponseClaimMatches(item)
  \/ IngressPacketPolicyRejected(item)

LeaderWirePacketExitReady(packet) ==
  LET recipient == packet.item.envelope.recipient
      transportSource == packet.item.source
      head == OldestDueSourcePacket(recipient, transportSource)
  IN /\ packet = head
     /\ LeaderWireIngressRemovalGate(head.item)

LeaderWirePacketHeadOfLineShadowed(packet) ==
  LET recipient == packet.item.envelope.recipient
      source == packet.item.source
  IN /\ DueSourcePackets(recipient, source) # {}
     /\ packet # OldestDueSourcePacket(recipient, source)

LeaderWireOverdueHeadOfLineShadowed(packet) ==
  LET recipient == packet.item.envelope.recipient
      source == packet.item.source
      head == OldestDueSourcePacket(recipient, source)
  IN /\ LeaderWirePacketHeadOfLineShadowed(packet)
     /\ head \in OverdueResponsivePackets

(***************************************************************************
For an authenticated response relayed by a non-timed outer source, the older
same-transport-source head need not itself satisfy `OverdueResponsivePackets`.
Such a head still shadows admission because `AdmitIngressPacket` chooses
`OldestDueSourcePacket`; admission then charges the selected item to its
normalized resource lane.  This is distinct from ordinary overdue FIFO debt.
***************************************************************************)
LeaderWireNonTimedOuterSourceShadowed(packet) ==
  LET recipient == packet.item.envelope.recipient
      source == packet.item.source
      head == OldestDueSourcePacket(recipient, source)
  IN /\ LeaderWirePacketHeadOfLineShadowed(packet)
     /\ source \notin AsyncTimedServiceNodes
     /\ head \notin OverdueResponsivePackets

LeaderWirePacketAdmissionGateBlocked(packet) ==
  LET recipient == packet.item.envelope.recipient
      transportSource == packet.item.source
      head == OldestDueSourcePacket(recipient, transportSource)
  IN /\ packet = head
     /\ ~LeaderWireIngressRemovalGate(head.item)

LeaderWireDueTransportResidual(packet) ==
  /\ packet \in OverdueResponsivePackets
  /\ LeaderWirePacket(packet)
  /\ \/ LeaderWireOverdueHeadOfLineShadowed(packet)
     \/ LeaderWireNonTimedOuterSourceShadowed(packet)
     \/ LeaderWirePacketAdmissionGateBlocked(packet)

LeaderWireRunnerReachRank(item) ==
  IF LeaderWireConsumerMilestone(item)
       \/ LeaderWireCandidateOwned(item)
  THEN 0
  ELSE IF LeaderWireIngressOwned(item)
       THEN CASE asyncRunnerPhase[item.envelope.recipient] = "Ingress" -> 1
              [] asyncRunnerPhase[item.envelope.recipient] = "Local" -> 2
              [] OTHER -> 3
       ELSE IF \E packet \in asyncTransport: packet.item = item
            THEN 4
            ELSE 5

LeaderWireRunnerAdmissionResidual(item) ==
  /\ LeaderWireItem(item)
  /\ LeaderWireIngressOwned(item)
  /\ ~LeaderWireCandidateOwned(item)
  /\ ~LeaderWireConsumerMilestone(item)
  /\ \/ asyncRunnerPhase[item.envelope.recipient] = "Runtime"
     \/ /\ asyncRunnerPhase[item.envelope.recipient] = "Local"
           /\ LocalAdmissionCanAdvance(item.envelope.recipient)
     \/ /\ asyncRunnerPhase[item.envelope.recipient] = "Ingress"
           /\ ~IngressItemCanDrain(
                item.envelope.recipient, item)

THEOREM DueLeaderWirePacketHasExitOrExactTransportResidual ==
  \A packet \in OverdueResponsivePackets:
    LeaderWirePacket(packet)
      => \/ LeaderWirePacketExitReady(packet)
         \/ LeaderWireDueTransportResidual(packet)
BY Isa
   DEF LeaderWireIngressRemovalGate,
       LeaderWirePacketExitReady,
       LeaderWireDueTransportResidual,
       LeaderWirePacketHeadOfLineShadowed,
       LeaderWireOverdueHeadOfLineShadowed,
       LeaderWireNonTimedOuterSourceShadowed,
       LeaderWirePacketAdmissionGateBlocked

THEOREM ExitReadyLeaderWireHeadEnablesExactPacketRemoval ==
  \A packet \in OverdueResponsivePackets:
    /\ AsyncStrongTypeInvariant
    /\ gst
    /\ LeaderWirePacket(packet)
    /\ LeaderWirePacketExitReady(packet)
    => ENABLED (
         PostGstAdmitHiddenPacket(
           packet.item.envelope.recipient, packet.item.source)
           /\ packet \notin asyncTransport')
BY AsyncStrongTypeProjectsAsyncType,
   GstResponsiveNodesAreUp,
   GstExcludesResponsiveReplayQuarantine,
   OldestDueSourcePacketFacts,
   ExpandENABLED, Isa
   DEF LeaderWirePacket, LeaderWireItem,
       LeaderWireIngressRemovalGate, LeaderWirePacketExitReady,
       PostGstAdmitHiddenPacket, AdmitIngressPacket,
       AdmitHiddenPacket, CoalesceHiddenPacket,
       DropPolicyRejectedHiddenPacket,
       DueSourcePackets, AsyncNonRunnerOuterFrame,
       AsyncNonCrashOuterFrame, AsyncCoreOuterFrame,
       AsyncAllVars, AsyncSchedulerVars, AsyncRecoveryVars,
       AsyncIoVars, AsyncDeferredVars, AsyncLocalAdmissionVars,
       LeaveCausalQueues, vars

THEOREM RuntimeResetLowersLeaderWireReachRank ==
  \A item \in AsyncNetworkItems,
     node \in AsyncCurrentResponsiveVoters:
    /\ LeaderWireItem(item)
    /\ item.envelope.recipient = node
    /\ LeaderWireIngressOwned(item)
    /\ ~LeaderWireCandidateOwned(item)
    /\ ~LeaderWireConsumerMilestone(item)
    /\ asyncRunnerPhase[node] = "Runtime"
    /\ SerializedRuntimeStep(node)
    => LeaderWireRunnerReachRank(item)'
         < LeaderWireRunnerReachRank(item)
BY Isa
   DEF LeaderWireRunnerReachRank,
       LeaderWireIngressOwned, LeaderWireCandidateOwned,
       LeaderWireConsumerMilestone,
       SerializedRuntimeStep, RuntimeStep,
       FifoRuntimeStep, DeferredDrainStep,
       IdleRuntimeStep, LeaveCausalQueues

THEOREM QuiescentLocalStepLowersLeaderWireReachRank ==
  \A item \in AsyncNetworkItems,
     node \in AsyncCurrentResponsiveVoters:
    /\ LeaderWireItem(item)
    /\ item.envelope.recipient = node
    /\ LeaderWireIngressOwned(item)
    /\ ~LeaderWireCandidateOwned(item)
    /\ ~LeaderWireConsumerMilestone(item)
    /\ asyncRunnerPhase[node] = "Local"
    /\ ~LocalAdmissionCanAdvance(node)
    /\ LocalAdmissionStep(node)
    => LeaderWireRunnerReachRank(item)'
         < LeaderWireRunnerReachRank(item)
BY Isa
   DEF LeaderWireRunnerReachRank,
       LeaderWireIngressOwned, LeaderWireCandidateOwned,
       LeaderWireConsumerMilestone,
       LocalAdmissionStep, LeaveCausalQueues

(***************************************************************************
Certified-response admission contract.

The authenticated archive identity and the outer relay route are independent
of bounded ingress ownership.  Every certified response is charged to the
aggregate untrusted resource source, has its own logical admission class, and
shares the one physical completion owner with Chunk.  Admission atomically
acquires the route-neutral exact-response claim, so archive rotation and relay
choice no longer create a refinement residual.

After admission, an exact claimed response reserves the runtime Completion
command directly.  It neither allocates a second effect-work owner nor passes
through a synthetic local-producer queue. Ordinary completions cannot consume
the final slot dedicated to `reserve_certified_body_available`; only a
physically full runtime may return retryable backpressure. That finite
serialized queue debt is independent of archive roster membership and relay
route and is the only response-specific state residual retained here.
***************************************************************************)

ProtectedExactCertifiedResponseOwned(item) ==
  /\ LeaderWireItem(item)
  /\ item.kind = "CertifiedResponse"
  /\ CertifiedResponseAuthorized(item)
  /\ ~LeaderWireCandidateOwned(item)
  /\ ~LeaderWireConsumerMilestone(item)
  /\ \/ \E packet \in asyncTransport: packet.item = item
     \/ LeaderWireIngressOwned(item)

CertifiedResponsePhysicalCompletionDebtResidual(item) ==
  LET node == item.envelope.recipient
  IN /\ ProtectedExactCertifiedResponseOwned(item)
     /\ CertifiedResponseClaimAuthorized(item)
     /\ IngressResourceSource(item) = AsyncUntrustedSource
     /\ IngressAdmissionClass(item) = "CertifiedResponse"
     /\ IngressUsesPhysicalCompletionOwner(item)
     /\ LeaderWireIngressOwned(item)
     /\ ~CanEnqueueCertifiedResponse(node)

(***************************************************************************
The temporal carrier is route-neutral.  The packet which first established
the claim may be coalesced away while another relay occurrence with the same
canonical signed envelope remains the one physical ingress owner.  Tracking
the exact outer-source item here would therefore manufacture an ownership
exit which neither consumes the claim nor advances consensus.
***************************************************************************)

CertifiedResponseClaimServiceResidual(item) ==
  /\ gst
  /\ LeaderWireCurrentContextWitnessIdentity(item)
  /\ CertifiedResponseClaimAuthorized(item)
  /\ CertifiedResponseClaimIngressOwner(
       AsyncCertifiedResponseAuthProjection(item))
  /\ IngressResourceSource(item) = AsyncUntrustedSource
  /\ IngressAdmissionClass(item) = "CertifiedResponse"
  /\ IngressUsesPhysicalCompletionOwner(item)
  /\ ~LeaderWireCandidateOwned(item)
  /\ ~LeaderWireConsumerMilestone(item)

THEOREM CertifiedResponsePhysicalDebtProjectsClaimServiceResidual ==
  \A item \in AsyncNetworkItems:
    /\ AsyncStrongTypeInvariant
    /\ gst
    /\ LeaderWireCurrentContextWitnessIdentity(item)
    /\ CertifiedResponsePhysicalCompletionDebtResidual(item)
    => CertifiedResponseClaimServiceResidual(item)
BY IsaT(180)
   DEF CertifiedResponsePhysicalCompletionDebtResidual,
       CertifiedResponseClaimServiceResidual,
       ProtectedExactCertifiedResponseOwned,
       LeaderWireIngressOwned,
       CertifiedResponseClaimIngressOwner,
       CertifiedResponseClaimMatches,
       IngressLaneDepth, SequenceSet

THEOREM CertifiedResponseClaimServiceProjectsRunnerOwned ==
  \A item \in AsyncNetworkItems:
    /\ AsyncStrongTypeInvariant
    /\ CertifiedResponseClaimServiceResidual(item)
    => /\ item.envelope.recipient \in ValidatorIds
       /\ CertifiedResponseClaimRunnerOwned(
            item.envelope.recipient)
BY AsyncCurrentResponsiveVotersAreValidators, IsaT(180)
   DEF CertifiedResponseClaimServiceResidual,
       LeaderWireCurrentContextWitnessIdentity,
       LeaderWireExactSemanticIdentity,
       LeaderWireItem,
       LeaderWireConsumerMilestone,
       CertifiedResponseClaimRunnerOwned,
       CertifiedResponseClaimAuthorized,
       CertifiedResponseClaimMatches,
       CertifiedResponseClaimsAt

THEOREM ExactCertifiedResponseUsesNormalizedPhysicalCompletionOwner ==
  \A item \in AsyncNetworkItems:
    ProtectedExactCertifiedResponseOwned(item)
      => /\ IngressResourceSource(item) = AsyncUntrustedSource
         /\ IngressAdmissionClass(item) = "CertifiedResponse"
         /\ IngressUsesPhysicalCompletionOwner(item)
BY Isa
   DEF ProtectedExactCertifiedResponseOwned,
       LeaderWireItem, IngressResourceSource,
       IngressUsesPhysicalCompletionOwner, IngressAdmissionClass

THEOREM CertifiedResponsePhysicalCompletionDebtBlocksExactDrain ==
  \A item \in AsyncNetworkItems:
    /\ AsyncStrongTypeInvariant
    /\ CertifiedResponsePhysicalCompletionDebtResidual(item)
    => ~IngressItemCanDrain(item.envelope.recipient, item)
BY AsyncStrongTypeProjectsAsyncType, Isa
   DEF CertifiedResponsePhysicalCompletionDebtResidual,
       ProtectedExactCertifiedResponseOwned,
       LeaderWireCandidateOwned,
       IngressItemCanDrain

(***************************************************************************
Timeout quorum and view rotation.

All-receipt quorum is strong enough for the model's dual quorum, and a
successful PersistInstallTC strictly advances the target view.  The residual
is therefore not quorum arithmetic.  It is the exact state in which a
responsive node has neither a Decision nor a certified future-view frontier,
while at least one responsive timeout signer is still absent.
***************************************************************************)

MissingResponsiveTimeoutSigners(recipient, roundView) ==
  {signer \in AsyncCurrentResponsiveVoters:
     ~ReceivedTimeoutVoteAt(recipient, signer, roundView)}

\* A global `formedTCs` occurrence is not a handoff to every validator.
\* Preserve the exact recipient through retained delivery, transport,
\* ingress, reducer ownership, or the recipient's install owner.
CertifiedFutureViewFrontier(node, roundView) ==
  TcFrontier(node, roundView)

TimeoutQuorumViewRotationResidual(node, roundView) ==
  /\ node \in AsyncCurrentResponsiveVoters
  /\ roundView = nodeView[node]
  /\ ~NodeHasDecision(node)
  /\ MissingResponsiveTimeoutSigners(node, roundView) # {}
  /\ ~CertifiedFutureViewFrontier(node, roundView)

THEOREM NoMissingResponsiveTimeoutSignerIsReceiptQuorum ==
  \A recipient \in ValidatorIds, roundView \in Views:
    MissingResponsiveTimeoutSigners(recipient, roundView) = {}
      <=> ResponsiveTimeoutReceiptQuorumAt(recipient, roundView)
BY Isa
   DEF MissingResponsiveTimeoutSigners,
       ResponsiveTimeoutReceiptQuorumAt,
       ReceivedTimeoutVoteAt

THEOREM ExactResponsiveTimeoutReceiptsSupplyDualQuorum ==
  \A recipient \in ValidatorIds, roundView \in Views:
    /\ TypeInvariant
    /\ ReceivedTimeoutVotePoolInvariant
    /\ MissingResponsiveTimeoutSigners(recipient, roundView) = {}
    => DualQuorum(
         CurrentEpoch,
         TimeoutSignerSet(TimeoutVotesAt(recipient, roundView)))
BY NoMissingResponsiveTimeoutSignerIsReceiptQuorum,
   ResponsiveReceiptsMakeDualQuorum

THEOREM PersistInstallStrictlyExitsItsTimeoutView ==
  \A candidate:
    /\ TypeInvariant
    /\ CandidateConsumerCurrent(candidate)
    /\ ExecutePersistInstall(candidate)
    => TimeoutViewGoal(candidate.node, candidate.view)'
BY ExecutePersistInstallAdvancesCertifiedView

(***************************************************************************
Complete exact current-context residual inventory.

These are current leader-corridor state/action predicates, not assumed
temporal closure:

  * an illegitimate same-consumer candidate discard;
  * a due leader wire occurrence hidden or admission-gated before ingress;
  * an ingress-owned item waiting for the runner/candidate handoff; or
  * an exact claimed certified response waiting for downstream physical
    completion capacity; or
  * a responsive view without a timeout quorum or certified future-view
    frontier.

The first residual is reduced above to an exact one-step safety property.
The wire, runner, response, and timeout corridors are outside
`CandidateServiceRank`.  The route-neutral certified-response claim and its
dedicated physical completion slot are discharged below by the generic fair
runner theorem plus an exact claim-exit safety kernel.  Transport, ordinary
runner admission, timeout/view rotation, and semantic induction over
`ExactLeaderCandidateRank` remain the open work before
`AdequateLeaderServiceKernelProperty(AsyncLiveSpecAt(...))` can be derived.
Naming the full disjunction prevents a proof of candidate starvation or the
now-closed response slot alone from being reported as adequate-leader
closure.
***************************************************************************)

AdequateLeaderExactPhysicalResidual ==
  \/ \E packet \in OverdueResponsivePackets:
       /\ LeaderWireCurrentContextWitnessIdentity(packet.item)
       /\ LeaderWireDueTransportResidual(packet)
  \/ \E item \in AsyncNetworkItems:
       /\ LeaderWireCurrentContextWitnessIdentity(item)
       /\ LeaderWireRunnerAdmissionResidual(item)
  \/ \E item \in AsyncNetworkItems:
       /\ LeaderWireCurrentContextWitnessIdentity(item)
       /\ CertifiedResponsePhysicalCompletionDebtResidual(item)
  \/ \E node \in AsyncCurrentResponsiveVoters,
       roundView \in Views:
       TimeoutQuorumViewRotationResidual(node, roundView)

AdequateLeaderExactSemanticResidual ==
  \/ \E candidate \in AsyncCandidateSet,
       rank \in (1..5) \X Nat:
       /\ ExactLeaderOwnerExitStep(candidate, rank)
       /\ UnexplainedSameConsumerLeaderDiscard(candidate, rank)
  \/ AdequateLeaderExactPhysicalResidual

THEOREM UnexplainedDiscardIsExactSemanticResidual ==
  \A candidate \in AsyncCandidateSet,
     rank \in (1..5) \X Nat:
    /\ ExactLeaderOwnerExitStep(candidate, rank)
    /\ UnexplainedSameConsumerLeaderDiscard(candidate, rank)
    => AdequateLeaderExactSemanticResidual
BY DEF AdequateLeaderExactSemanticResidual

THEOREM BlockedLeaderWireIsExactSemanticResidual ==
  \A packet \in OverdueResponsivePackets:
    /\ LeaderWireCurrentContextWitnessIdentity(packet.item)
    /\ LeaderWireDueTransportResidual(packet)
      => AdequateLeaderExactSemanticResidual
BY DEF AdequateLeaderExactSemanticResidual

THEOREM RunnerAdmissionGapIsExactSemanticResidual ==
  \A item \in AsyncNetworkItems:
    /\ LeaderWireCurrentContextWitnessIdentity(item)
    /\ LeaderWireRunnerAdmissionResidual(item)
      => AdequateLeaderExactSemanticResidual
BY DEF AdequateLeaderExactSemanticResidual

THEOREM CertifiedResponsePhysicalCompletionDebtIsExactSemanticResidual ==
  \A item \in AsyncNetworkItems:
    /\ LeaderWireCurrentContextWitnessIdentity(item)
    /\ CertifiedResponsePhysicalCompletionDebtResidual(item)
      => AdequateLeaderExactSemanticResidual
BY DEF AdequateLeaderExactSemanticResidual

THEOREM TimeoutQuorumGapIsExactSemanticResidual ==
  \A node \in AsyncCurrentResponsiveVoters,
     roundView \in Views:
    TimeoutQuorumViewRotationResidual(node, roundView)
      => AdequateLeaderExactSemanticResidual
BY DEF AdequateLeaderExactSemanticResidual

(***************************************************************************
Exact temporal sub-kernels for every state-level residual arm.

The first residual arm above is an action-safety defect and is discharged by
`ExactLeaderSchedulerOriginReadinessProperty`; it is not incorrectly coerced
into a state predicate under `~>`.  The four predicates below are the exact
physical handoffs for the remaining state-level arms:

  * transport transfers the selected item to ingress, a reducer candidate,
    or a concrete consumer receipt;
  * runner admission removes the exact ingress owner or creates the candidate
    or receipt;
  * certified-response capacity does the same after the linear claim; and
  * timeout collection reaches receipt quorum, a certified future-view
    frontier, a strict view advance, or Decision.

The complete property remains deliberately split.  The exact
certified-response arm is proved below; the open physical property retains
transport, ordinary runner admission, and timeout/view rotation.  Together
they form the full off-scheduler property consumed by the semantic reduction,
with action-safety readiness bundled separately.

Transport policy is deliberately not split into temporal branches.
`IngressPacketPolicyRejected` depends on mutable request/claim state.
Cleanup therefore converges to one combined handoff-or-exact-absence target.
The productive promise uses an immutable item classifier which excludes
generic untrusted transport completions; if its certified-response policy
later rejects, the response already has an exact candidate or consumer
milestone.
***************************************************************************)

LeaderWireTransportHandoff(packet) ==
  /\ packet \in AsyncPacketSet
  /\ LeaderWireCurrentContextWitnessIdentity(packet.item)
  /\ \/ LeaderWireIngressOwned(packet.item)
     \/ LeaderWireCandidateOwned(packet.item)
     \/ LeaderWireConsumerMilestone(packet.item)

\* Exact occurrence retirement is useful for nonproductive cleanup traffic.
\* It is intentionally not accepted as progress for the stable productive
\* transport classifier above.
LeaderWireTransportOccurrenceAbsent(packet) ==
  /\ packet \in AsyncPacketSet
  /\ packet \notin asyncTransport

LeaderWireTransportResolution(packet) ==
  \/ LeaderWireTransportHandoff(packet)
  \/ LeaderWireTransportOccurrenceAbsent(packet)

LeaderWireRunnerAdmissionHandoff(item) ==
  /\ LeaderWireCurrentContextWitnessIdentity(item)
  /\ \/ LeaderWireCandidateOwned(item)
     \/ LeaderWireConsumerMilestone(item)

CertifiedResponsePhysicalCompletionHandoff(item) ==
  /\ LeaderWireCurrentContextWitnessIdentity(item)
  /\ item.kind = "CertifiedResponse"
  /\ \/ LeaderWireCandidateOwned(item)
     \/ LeaderWireConsumerMilestone(item)

THEOREM PolicyRejectedLeaderWireAlreadyHasSemanticHandoff ==
  \A packet \in AsyncPacketSet:
    /\ LeaderWireProductiveTransportIdentity(packet.item)
    /\ IngressPacketPolicyRejected(packet.item)
    => LeaderWireTransportHandoff(packet)
PROOF
  <1>1. ASSUME NEW packet \in AsyncPacketSet,
                LeaderWireProductiveTransportIdentity(packet.item),
                IngressPacketPolicyRejected(packet.item)
         PROVE LeaderWireTransportHandoff(packet)
    <2>1. /\ LeaderWireCurrentContextWitnessIdentity(packet.item)
           /\ ~(/\ IngressAdmissionClass(packet.item) =
                    "TransportCompletion"
                /\ IngressResourceSource(packet.item) =
                    AsyncUntrustedSource)
      BY <1>1 DEF LeaderWireProductiveTransportIdentity
    <2>2. ~UntrustedGenericCompletionPacketPolicyRejected(packet.item)
      BY <2>1 DEF UntrustedGenericCompletionPacketPolicyRejected
    <2>3. CertifiedResponsePacketPolicyRejected(packet.item)
      BY <1>1, <2>2 DEF IngressPacketPolicyRejected
    <2>4. /\ packet.item.kind = "CertifiedResponse"
           /\ ~CertifiedResponseAuthorized(packet.item)
      BY <2>3 DEF CertifiedResponsePacketPolicyRejected
    <2>5. LeaderCertifiedResponseRelevant(packet.item)
      BY <2>1, <2>4
         DEF LeaderWireCurrentContextWitnessIdentity,
             LeaderWireExactSemanticIdentity, LeaderWireItem
    <2>6. \/ CandidateScheduled(
                CertifiedResponseCandidate(packet.item))
           \/ SameIdentityLeaderOwner(
                CertifiedResponseCandidate(packet.item))
           \/ BodyHeldBy(durableBodies,
                         packet.item.envelope.recipient, context,
                         packet.item.envelope.view,
                         packet.item.envelope.subject)
           \/ nodeView[packet.item.envelope.recipient]
                > packet.item.envelope.view
           \/ NodeHasDecision(packet.item.envelope.recipient)
      BY <2>4, <2>5, Isa DEF LeaderCertifiedResponseRelevant
    <2>7. \/ LeaderWireCandidateOwned(packet.item)
           \/ LeaderWireConsumerMilestone(packet.item)
      BY <2>4, <2>6, Isa
         DEF LeaderWireCandidateOwned, LeaderWireConsumerMilestone
    <2> QED BY <1>1, <2>1, <2>7
         DEF LeaderWireTransportHandoff
  <1> QED BY <1>1

(***************************************************************************
Exact claim-exit safety.

The generic fair-runner theorem is recipient-local: it proves that the one
linear response claim disappears or the recipient applies.  To specialize
that theorem to this frozen leader occurrence, every exit from the
route-neutral claim carrier must expose the corresponding semantic handoff.
The higher-view arm in `LeaderWireConsumerMilestone` is essential here:
PersistInstallTC may intentionally retire an obsolete body request, which is
pacemaker progress rather than a lost response.
***************************************************************************)

CertifiedResponseClaimServicePersistsOrHandoff(item) ==
  CertifiedResponseClaimServiceResidual(item)
    => \/ CertifiedResponseClaimServiceResidual(item)'
       \/ CertifiedResponsePhysicalCompletionHandoff(item)'

THEOREM CertifiedResponseClaimServiceResidualStepIsSafe ==
  \A item \in AsyncNetworkItems:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ DecisionFrontierUniquenessInvariant
    /\ DecisionTimeoutFrontierInvariant
    /\ ResponsiveRecoveryValidationClearedInvariant
    /\ FinalProgressWitnessClosureInvariant
    /\ [AsyncNext]_AsyncAllVars
    => CertifiedResponseClaimServicePersistsOrHandoff(item)
BY MatchingClaimedCertifiedResponseIsAuthorized,
   FreshAuthorizedCertifiedResponseSchedulerFrame,
   ScheduledAuthorizedCertifiedResponseSchedulerFrame,
   AuthorizedCertifiedResponseFrame,
   DrainFairIngressSelectedClaimPopShape,
   AsyncBracketStepLeavesContext,
   AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesFinalProgressWitnessClosure,
   GstAsyncStepIsMonotone, IsaT(300)
   DEF CertifiedResponseClaimServicePersistsOrHandoff,
       CertifiedResponseClaimServiceResidual,
       CertifiedResponsePhysicalCompletionHandoff,
       LeaderWireCurrentContextWitnessIdentity,
       LeaderWireExactSemanticIdentity,
       LeaderWireItem, LeaderCertifiedResponseRelevant,
       LeaderWireCarriesContext,
       LeaderWireCandidateOwned,
       LeaderWireConsumerMilestone,
       SameIdentityLeaderOwner,
       ExactLeaderCandidateRank,
       ResponsiveProtectedCandidateOwned,
       CertifiedResponseClaimIngressOwner,
       CertifiedResponseClaimAuthorized,
       CertifiedResponseClaimMatches,
       CertifiedResponseClaimsAt,
       CertifiedResponseClaimForRequests,
       ActiveCertifiedRequestHashesIn,
       AsyncCertifiedRequestHash,
       AsyncCertifiedResponseAuthProjection,
       AsyncCertifiedResponseCanonicalWireIdentity,
       IngressResourceSource,
       IngressAdmissionClass,
       IngressUsesPhysicalCompletionOwner,
       CertifiedResponseCandidate,
       CandidateScheduled,
       SequenceSet

CertifiedResponseClaimServiceExitSafetyKernelProperty(specification) ==
  specification
    => \A item \in AsyncNetworkItems:
         [][CertifiedResponseClaimServicePersistsOrHandoff(
              item)]_AsyncAllVars

THEOREM CertifiedResponseClaimServiceExitSafetyKernel ==
  \A initialContext:
    CertifiedResponseClaimServiceExitSafetyKernelProperty(
      AsyncSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE CertifiedResponseClaimServiceExitSafetyKernelProperty(
                 AsyncSpecAt(initialContext))
    <2>1. AsyncSpecAt(initialContext)
             => [](/\ AsyncStrongTypeInvariant
                    /\ AsyncProgressOwnershipInvariant
                    /\ DecisionFrontierUniquenessInvariant
                    /\ DecisionTimeoutFrontierInvariant
                    /\ ResponsiveRecoveryValidationClearedInvariant
                    /\ FinalProgressWitnessClosureInvariant)
      BY AsyncSpecAlwaysStrongTypeInvariant,
         AsyncSpecAlwaysProgressOwnershipInvariant,
         DecisionFrontierUniquenessInvariantFromAsyncSpec,
         DecisionTimeoutFrontierInvariantFromAsyncSpec,
         ResponsiveRecoveryValidationClearedInvariantObligation,
         FinalProgressWitnessClosureInvariantObligation, PTL
    <2>2. \A item \in AsyncNetworkItems:
             /\ AsyncStrongTypeInvariant
             /\ AsyncProgressOwnershipInvariant
             /\ DecisionFrontierUniquenessInvariant
             /\ DecisionTimeoutFrontierInvariant
             /\ ResponsiveRecoveryValidationClearedInvariant
             /\ FinalProgressWitnessClosureInvariant
             /\ [AsyncNext]_AsyncAllVars
             => [CertifiedResponseClaimServicePersistsOrHandoff(
                   item)]_AsyncAllVars
      BY CertifiedResponseClaimServiceResidualStepIsSafe
    <2>3. AsyncSpecAt(initialContext)
             => [][AsyncNext]_AsyncAllVars
      BY DEF AsyncSpecAt
    <2> QED BY <2>1, <2>2, <2>3, PTL
         DEF CertifiedResponseClaimServiceExitSafetyKernelProperty
  <1> QED BY <1>1

CertifiedResponseClaimServiceConvergenceProperty(specification) ==
  specification
    => \A item \in AsyncNetworkItems:
         CertifiedResponseClaimServiceResidual(item)
           ~> CertifiedResponsePhysicalCompletionHandoff(item)

THEOREM CertifiedResponseClaimExitSafetyDischargesServiceResidual ==
  \A initialContext:
    CertifiedResponseClaimServiceExitSafetyKernelProperty(
      AsyncSpecAt(initialContext))
      => CertifiedResponseClaimServiceConvergenceProperty(
           AsyncSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext,
                CertifiedResponseClaimServiceExitSafetyKernelProperty(
                  AsyncSpecAt(initialContext))
         PROVE CertifiedResponseClaimServiceConvergenceProperty(
                 AsyncSpecAt(initialContext))
    <2>1. ASSUME AsyncSpecAt(initialContext)
           PROVE \A item \in AsyncNetworkItems:
                    CertifiedResponseClaimServiceResidual(item)
                      ~> CertifiedResponsePhysicalCompletionHandoff(item)
      <3>1. AsyncSpecAt(initialContext)
               => []AsyncStrongTypeInvariant
        BY AsyncSpecAlwaysStrongTypeInvariant
      <3>2. ASSUME NEW item \in AsyncNetworkItems
             PROVE CertifiedResponseClaimServiceResidual(item)
                     ~> CertifiedResponsePhysicalCompletionHandoff(item)
        <4>1. [](CertifiedResponseClaimServiceResidual(item)
                  => /\ item.envelope.recipient \in ValidatorIds
                     /\ gst
                     /\ CertifiedResponseClaimRunnerOwned(
                          item.envelope.recipient))
          BY <2>1, <3>1,
             CertifiedResponseClaimServiceProjectsRunnerOwned, PTL
             DEF CertifiedResponseClaimServiceResidual
        <4>2. CertifiedResponseClaimServiceResidual(item)
                 ~> CertifiedResponseClaimRunnerGoal(
                      item.envelope.recipient)
          BY <2>1, <4>1,
             GstCertifiedResponseClaimRunnerConvergence, PTL
        <4>3. [][CertifiedResponseClaimServicePersistsOrHandoff(
                    item)]_AsyncAllVars
          BY <1>1, <2>1
             DEF CertifiedResponseClaimServiceExitSafetyKernelProperty
        <4>4. /\ AsyncStrongTypeInvariant
                /\ CertifiedResponseClaimServiceResidual(item)
                /\ CertifiedResponseClaimRunnerGoal(
                     item.envelope.recipient)
               => CertifiedResponsePhysicalCompletionHandoff(item)
          BY Isa
             DEF CertifiedResponseClaimServiceResidual,
                 CertifiedResponsePhysicalCompletionHandoff,
                 LeaderWireConsumerMilestone,
                 CertifiedResponseClaimRunnerGoal,
                 CertifiedResponseClaimRunnerOwned,
                 CertifiedResponseClaimAuthorized,
                 CertifiedResponseClaimMatches,
                 CertifiedResponseClaimsAt
        <4> QED BY <3>1, <4>2, <4>3, <4>4, PTL
             DEF CertifiedResponseClaimServicePersistsOrHandoff
      <3> QED BY <3>2
    <2> QED BY <2>1
         DEF CertifiedResponseClaimServiceConvergenceProperty
  <1> QED BY <1>1

THEOREM CertifiedResponseClaimServiceConvergence ==
  \A initialContext:
    CertifiedResponseClaimServiceConvergenceProperty(
      AsyncSpecAt(initialContext))
BY CertifiedResponseClaimServiceExitSafetyKernel,
   CertifiedResponseClaimExitSafetyDischargesServiceResidual

CertifiedResponsePhysicalDebtConvergenceProperty(specification) ==
  specification
    => \A item \in AsyncNetworkItems:
         (gst
           /\ LeaderWireCurrentContextWitnessIdentity(item)
           /\ CertifiedResponsePhysicalCompletionDebtResidual(item))
           ~> (ResponsiveNodesDecide
                \/ CertifiedResponsePhysicalCompletionHandoff(item))

THEOREM CertifiedResponsePhysicalDebtConvergence ==
  \A initialContext:
    CertifiedResponsePhysicalDebtConvergenceProperty(
      AsyncLiveSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE CertifiedResponsePhysicalDebtConvergenceProperty(
                 AsyncLiveSpecAt(initialContext))
    <2>1. CertifiedResponseClaimServiceConvergenceProperty(
             AsyncSpecAt(initialContext))
      BY CertifiedResponseClaimServiceConvergence
    <2>2. AsyncLiveSpecAt(initialContext)
            => AsyncSpecAt(initialContext)
      BY AsyncLiveSpecProjectsAsyncSpec
    <2>3. AsyncSpecAt(initialContext)
            => []AsyncStrongTypeInvariant
      BY AsyncSpecAlwaysStrongTypeInvariant
    <2>4. ASSUME AsyncLiveSpecAt(initialContext)
           PROVE \A item \in AsyncNetworkItems:
                    (gst
                      /\ LeaderWireCurrentContextWitnessIdentity(item)
                      /\ CertifiedResponsePhysicalCompletionDebtResidual(
                           item))
                      ~> (ResponsiveNodesDecide
                           \/ CertifiedResponsePhysicalCompletionHandoff(
                                item))
      <3>1. ASSUME NEW item \in AsyncNetworkItems
             PROVE (gst
                     /\ LeaderWireCurrentContextWitnessIdentity(item)
                     /\ CertifiedResponsePhysicalCompletionDebtResidual(
                          item))
                     ~> (ResponsiveNodesDecide
                          \/ CertifiedResponsePhysicalCompletionHandoff(
                               item))
        <4>1. [](gst
                   /\ LeaderWireCurrentContextWitnessIdentity(item)
                   /\ CertifiedResponsePhysicalCompletionDebtResidual(item)
                  => CertifiedResponseClaimServiceResidual(item))
          BY <2>2, <2>3, <2>4,
             CertifiedResponsePhysicalDebtProjectsClaimServiceResidual,
             PTL
        <4>2. CertifiedResponseClaimServiceResidual(item)
                 ~> CertifiedResponsePhysicalCompletionHandoff(item)
          BY <2>1, <2>2, <2>4
             DEF CertifiedResponseClaimServiceConvergenceProperty
        <4> QED BY <4>1, <4>2, PTL
      <3> QED BY <3>1
    <2> QED BY <2>4
         DEF CertifiedResponsePhysicalDebtConvergenceProperty
  <1> QED BY <1>1

TimeoutQuorumViewRotationHandoff(node, roundView) ==
  /\ node \in AsyncCurrentResponsiveVoters
  /\ roundView \in Views
  /\ \/ NodeHasDecision(node)
     \/ /\ roundView = nodeView[node]
           /\ \/ CertifiedFutureViewFrontier(node, roundView)
              \/ MissingResponsiveTimeoutSigners(node, roundView) = {}
     \/ \E installed \in installedTCs:
          /\ installed.node = node
          /\ installed.tc.context = context
          /\ installed.tc.view >= roundView
          /\ nodeView[node] = installed.tc.view + 1

AdequateLeaderExactPhysicalHandoff ==
  \/ \E packet \in AsyncPacketSet:
       LeaderWireTransportHandoff(packet)
  \/ \E item \in AsyncNetworkItems:
       LeaderWireRunnerAdmissionHandoff(item)
  \/ \E item \in AsyncNetworkItems:
       CertifiedResponsePhysicalCompletionHandoff(item)
  \/ \E node \in ValidatorIds, roundView \in Views:
       TimeoutQuorumViewRotationHandoff(node, roundView)

AdequateLeaderProductivePhysicalResidual ==
  \/ \E packet \in OverdueResponsivePackets:
       /\ LeaderWireProductiveTransportIdentity(packet.item)
       /\ LeaderWireDueTransportResidual(packet)
  \/ \E item \in AsyncNetworkItems:
       /\ LeaderWireCurrentContextWitnessIdentity(item)
       /\ LeaderWireRunnerAdmissionResidual(item)
  \/ \E item \in AsyncNetworkItems:
       /\ LeaderWireCurrentContextWitnessIdentity(item)
       /\ CertifiedResponsePhysicalCompletionDebtResidual(item)
  \/ \E node \in AsyncCurrentResponsiveVoters,
       roundView \in Views:
       TimeoutQuorumViewRotationResidual(node, roundView)

AdequateLeaderOpenPhysicalResidualConvergenceProperty(specification) ==
  specification
    => /\ \A packet \in AsyncPacketSet:
             (gst
               /\ LeaderWireCurrentContextWitnessIdentity(packet.item)
               /\ LeaderWireDueTransportResidual(packet))
               ~> (ResponsiveNodesDecide
                    \/ LeaderWireTransportResolution(packet))
       /\ \A packet \in AsyncPacketSet:
             (gst
               /\ LeaderWireProductiveTransportIdentity(packet.item)
               /\ LeaderWireDueTransportResidual(packet))
               ~> (ResponsiveNodesDecide
                    \/ LeaderWireTransportHandoff(packet))
       /\ \A item \in AsyncNetworkItems:
             (gst
               /\ LeaderWireCurrentContextWitnessIdentity(item)
               /\ LeaderWireRunnerAdmissionResidual(item))
               ~> (ResponsiveNodesDecide
                    \/ LeaderWireRunnerAdmissionHandoff(item))
       /\ \A node \in ValidatorIds,
             roundView \in Views:
             (gst
               /\ TimeoutQuorumViewRotationResidual(node, roundView))
               ~> (ResponsiveNodesDecide
                    \/ TimeoutQuorumViewRotationHandoff(node, roundView))

AdequateLeaderExactPhysicalResidualConvergenceProperty(specification) ==
  /\ AdequateLeaderOpenPhysicalResidualConvergenceProperty(
       specification)
  /\ CertifiedResponsePhysicalDebtConvergenceProperty(specification)

AdequateLeaderExactResidualKernelProperty(specification) ==
  /\ ExactLeaderSchedulerOriginReadinessProperty(specification)
  /\ AdequateLeaderOpenPhysicalResidualConvergenceProperty(specification)

THEOREM ExactResidualKernelSuppliesExactPhysicalConvergence ==
  \A initialContext:
    AdequateLeaderExactResidualKernelProperty(
      AsyncLiveSpecAt(initialContext))
      => AdequateLeaderExactPhysicalResidualConvergenceProperty(
           AsyncLiveSpecAt(initialContext))
BY CertifiedResponsePhysicalDebtConvergence
   DEF AdequateLeaderExactResidualKernelProperty,
       AdequateLeaderExactPhysicalResidualConvergenceProperty

THEOREM ExactResidualKernelSuppliesCandidateSemanticHandoffs ==
  \A initialContext:
    AdequateLeaderExactResidualKernelProperty(
      AsyncLiveSpecAt(initialContext))
      => ExactLeaderCandidateSemanticHandoffProperty(
           AsyncLiveSpecAt(initialContext))
BY SchedulerOriginReadinessReducesToExactLeaderExitSafety,
   ExactDiscardSafetyClosesAdmittedCandidateHandoffs
   DEF AdequateLeaderExactResidualKernelProperty

(***************************************************************************
One exact semantic-composition kernel.

Physical service and candidate starvation do not by themselves establish
that a view advance selects the next useful leader, nor that a successful
candidate milestone is the next lower proposal/Prepare/Commit/Decision
frontier.  The following single property isolates precisely that semantic
composition:

  1. each release antecedent exposes a terminal result, one concrete physical
     residual, or one ranked exact candidate;
  2. each completed physical handoff exposes a terminal result or a ranked
     candidate; and
  3. each actual current owned rank frontier persists across a step or the
     step exposes a terminal result or a strictly lower semantic rank.

No convergence theorem for this property is asserted.  Its rank relation is
the finite lexicographic product of phase 1..5 and the concrete inner
positions 0..9, so the reduction below uses ordinary well-founded induction
rather than assuming the aggregate leader-service conclusion.

The source exposure is necessarily stated at each release antecedent.  Every
non-terminal target below is nevertheless narrower: a wire handoff retains
one fixed packet/item plus its current context, responsive recipient, view,
and subject; the candidate step retains the actual current owned candidate,
its frozen consumer context, responsive witness, protocol view, subject, and
exact semantic rank.  Thus a historical packet cannot satisfy a
current-context wire handoff, and a fabricated stale candidate cannot replace
the candidate chosen at a rank frontier.
***************************************************************************)

AdequateLeaderCompositionModes ==
  {"ReachAdequateView", "DecideFromAdequateView"}

AdequateLeaderModeGoal(mode) ==
  CASE mode = "ReachAdequateView" ->
         (AdequateResponsiveHonestLeaderViewReached
            \/ ResponsiveNodesDecide)
    [] OTHER -> ResponsiveNodesDecide

AdequateLeaderModeSource(mode) ==
  CASE mode = "ReachAdequateView" ->
         (gst /\ ~ResponsiveNodesDecide)
    [] OTHER ->
         (gst /\ AdequateResponsiveHonestLeaderViewReached
                /\ ~ResponsiveNodesDecide)

\* A mode remains active after its source has fired until its release goal is
\* observed.  The Decide-mode rank frontier below still requires a concrete
\* current adequate-leader identity; keeping the temporal obligation active
\* does not let an unrelated candidate stand in for that leader corridor.
AdequateLeaderModeActive(mode) ==
  /\ gst
  /\ ~AdequateLeaderModeGoal(mode)

ExactLeaderSemanticRankCarrier == (1..5) \X (0..9)

ExactLeaderStaticSemanticRank(candidate, rank) ==
  /\ rank \in ExactLeaderSemanticRankCarrier
  /\ \/ /\ candidate.kind = "PersistTimeout"
        /\ rank = ViewChangeSemanticRank(7)
     \/ /\ candidate.kind = "SignTimeout"
        /\ rank = ViewChangeSemanticRank(6)
     \/ /\ candidate.kind = "DeliverTimeout"
        /\ candidate.item.kind = "TimeoutVote"
        /\ rank = ViewChangeSemanticRank(5)
     \/ /\ candidate.kind = "FormTC"
        /\ rank = ViewChangeSemanticRank(4)
     \/ /\ candidate.kind = "DeliverTC"
        /\ candidate.item.kind = "TimeoutCertificate"
        /\ rank = ViewChangeSemanticRank(4)
     \/ /\ candidate.kind = "BeginInstallTC"
        /\ rank = ViewChangeSemanticRank(3)
     \/ /\ candidate.kind = "PersistInstallTC"
        /\ rank = ViewChangeSemanticRank(2)
     \/ /\ candidate.kind = "AssembleBody"
        /\ rank = ProposalSemanticRank(9)
     \/ /\ candidate.kind = "BeginProposal"
        /\ rank = ProposalSemanticRank(8)
     \/ /\ candidate.kind = "PersistProposal"
        /\ rank = ProposalSemanticRank(7)
     \/ /\ candidate.kind = "SignProposal"
        /\ rank = ProposalSemanticRank(6)
     \/ /\ candidate.kind = "DeliverProposal"
        /\ candidate.item.kind = "Proposal"
        /\ rank = ProposalSemanticRank(5)
     \/ /\ candidate.kind = "DeliverChunk"
        /\ candidate.item.kind = "Chunk"
        /\ rank = ProposalSemanticRank(5)
     \/ /\ candidate.kind
              \in {"FetchBody", "RebindRetainedBody",
                   "FetchCertifiedBody"}
        /\ rank = ProposalSemanticRank(4)
     \/ /\ candidate.kind = "StoreBody"
        /\ rank = ProposalSemanticRank(3)
     \/ /\ candidate.kind = "ValidateBody"
        /\ rank = ProposalSemanticRank(2)
     \/ /\ candidate.kind = "BeginPrepare"
        /\ rank = PrepareSemanticRank(9)
     \/ /\ candidate.kind = "PersistPrepare"
        /\ rank = PrepareSemanticRank(8)
     \/ /\ candidate.kind = "SignVote"
        /\ rank \in {PrepareSemanticRank(7), CommitSemanticRank(7)}
     \/ /\ candidate.kind = "DeliverVote"
        /\ candidate.item.kind = "PrepareVote"
        /\ rank = PrepareSemanticRank(6)
     \/ /\ candidate.kind = "FormPrepareQC"
        /\ rank = PrepareSemanticRank(5)
     \/ /\ candidate.kind = "DeliverQC"
        /\ candidate.item.kind = "PrepareQC"
        /\ rank = PrepareSemanticRank(4)
     \/ /\ candidate.kind = "BeginObservePrepare"
        /\ rank = PrepareSemanticRank(3)
     \/ /\ candidate.kind = "PersistObservePrepare"
        /\ rank = PrepareSemanticRank(2)
     \/ /\ candidate.kind = "BeginLockCommit"
        /\ rank = CommitSemanticRank(9)
     \/ /\ candidate.kind = "PersistLockCommit"
        /\ rank = CommitSemanticRank(8)
     \/ /\ candidate.kind = "DeliverVote"
        /\ candidate.item.kind = "CommitVote"
        /\ rank = CommitSemanticRank(6)
     \/ /\ candidate.kind = "FormCommitQC"
        /\ rank = CommitSemanticRank(5)
     \/ /\ candidate.kind = "DeliverQC"
        /\ candidate.item.kind = "CommitQC"
        /\ rank = CommitSemanticRank(4)
     \/ /\ candidate.kind = "BeginDecision"
        /\ rank = DecisionSemanticRank(3)
     \/ /\ candidate.kind = "PersistDecision"
        /\ rank = DecisionSemanticRank(2)

THEOREM ExactLeaderCandidateRankProjectsStaticSemanticRank ==
  \A candidate, rank:
    ExactLeaderCandidateRank(candidate, rank)
      => ExactLeaderStaticSemanticRank(candidate, rank)
BY Isa
   DEF ExactLeaderCandidateRank, ExactLeaderStaticSemanticRank,
       ExactLeaderSemanticRankCarrier,
       ViewChangeSemanticRank, ProposalSemanticRank,
       PrepareSemanticRank, CommitSemanticRank, DecisionSemanticRank

ExactLeaderFrozenSemanticIdentity(
    candidate, rank, leaderContext, witness, roundView, subject) ==
  /\ leaderContext \in ContextRecords
  /\ witness \in Responsive \cap VotingRoster(leaderContext.epoch)
  /\ roundView \in Views
  /\ subject \in SubjectOrNone
  /\ candidate.node = witness
  /\ candidate.height = leaderContext.height
  /\ candidate.consumerContext = leaderContext
  /\ candidate.view = roundView
  /\ candidate.subject = subject
  /\ ExactLeaderStaticSemanticRank(candidate, rank)
  /\ IF candidate.kind = "AssembleBody"
     THEN Leader(leaderContext, roundView) = witness
     ELSE TRUE

ExactLeaderCurrentRankWitness(
    candidate, rank, leaderContext, witness, roundView, subject) ==
  /\ leaderContext = context
  /\ candidate.consumerView = nodeView[witness]
  /\ candidate.consumerGeneration = generation[witness]
  /\ ExactLeaderFrozenSemanticIdentity(
       candidate, rank, leaderContext, witness, roundView, subject)
  /\ ExactLeaderCandidateRank(candidate, rank)

ExactAdequateLeaderViewIdentity(leader, roundView) ==
  /\ leader \in AsyncCurrentResponsiveVoters \cap Honest
  /\ roundView \in Views
  /\ nodeView[leader] = roundView
  /\ ~NodeHasDecision(leader)
  /\ Leader(context, roundView) = leader
  /\ AsyncViewTimeout(roundView) > AsyncWorstCaseServiceBudget

ExactLeaderSemanticRankOrdering ==
  LexPairOrdering(
    OpToRel(<, Nat), OpToRel(<, Nat), 1..5, 0..9)

AdequateLeaderModeRankFrontier(mode, rank) ==
  /\ AdequateLeaderModeActive(mode)
  /\ \E candidate \in AsyncCandidateSet,
        leaderContext \in ContextRecords,
        witness \in ValidatorIds,
        roundView \in Views,
        subject \in SubjectOrNone:
       /\ ExactLeaderCurrentRankWitness(
            candidate, rank, leaderContext, witness, roundView, subject)
       /\ IF mode = "DecideFromAdequateView"
          THEN \E leader \in ValidatorIds,
                 leaderView \in Views:
                 /\ ExactAdequateLeaderViewIdentity(leader, leaderView)
                 /\ candidate.view = leaderView
                 /\ candidate.consumerView = leaderView
          ELSE TRUE

(***************************************************************************
Owned rank-step anchor.

The old composition family quantified an arbitrary candidate and used
`ExactLeaderCandidateExitOutcome` as a state antecedent.  Because that outcome
contains `~CandidateConsumerCurrent`, an unowned fabricated stale
PersistDecision record at rank <<1, 2>> could trigger the family; ranks
<<1, 0>> and <<1, 1>> have no candidate constructors, so the family collapsed
to the terminal Decision property.  The action predicate below instead
anchors every step to the exact current owned candidate which witnesses the
mode frontier.  It says that one concrete bracketed step either preserves
that same anchor, reaches the mode goal, or exposes a strictly lower owned
frontier.
***************************************************************************)

ExactLeaderModeRankAnchor(
    mode, candidate, rank, leaderContext, witness, roundView, subject) ==
  /\ AdequateLeaderModeActive(mode)
  /\ ExactLeaderCurrentRankWitness(
       candidate, rank, leaderContext, witness, roundView, subject)
  /\ IF mode = "DecideFromAdequateView"
     THEN \E leader \in ValidatorIds,
             leaderView \in Views:
            /\ ExactAdequateLeaderViewIdentity(leader, leaderView)
            /\ candidate.view = leaderView
            /\ candidate.consumerView = leaderView
     ELSE TRUE

ExactLeaderAnchoredRankProgressStep(
    mode, candidate, rank, leaderContext, witness, roundView, subject) ==
  ExactLeaderModeRankAnchor(
    mode, candidate, rank, leaderContext, witness, roundView, subject)
    => \/ ExactLeaderModeRankAnchor(
            mode, candidate, rank, leaderContext,
            witness, roundView, subject)'
       \/ AdequateLeaderModeGoal(mode)'
       \/ \E lowerRank \in
              SetLessThan(
                rank,
                ExactLeaderSemanticRankOrdering,
                ExactLeaderSemanticRankCarrier):
            AdequateLeaderModeRankFrontier(mode, lowerRank)'

THEOREM FabricatedStaleUnownedPersistDecisionCannotTriggerRankStep ==
  \A mode \in AdequateLeaderCompositionModes,
     candidate,
     rank \in ExactLeaderSemanticRankCarrier,
     leaderContext \in ContextRecords,
     witness \in ValidatorIds,
     roundView \in Views,
     subject \in SubjectOrNone:
    /\ candidate.kind = "PersistDecision"
    /\ ~ResponsiveProtectedCandidateOwned(candidate)
    => ~ExactLeaderModeRankAnchor(
          mode, candidate, rank, leaderContext,
          witness, roundView, subject)
BY ExactLeaderCandidateRankIsSemanticRank, Isa
   DEF ExactLeaderModeRankAnchor,
       ExactLeaderCurrentRankWitness

THEOREM ExactLeaderCandidateRankIsInBoundedSemanticCarrier ==
  \A candidate, rank:
    ExactLeaderCandidateRank(candidate, rank)
      => rank \in ExactLeaderSemanticRankCarrier
BY Isa
   DEF ExactLeaderCandidateRank, ExactLeaderSemanticRankCarrier,
       ViewChangeSemanticRank, ProposalSemanticRank,
       PrepareSemanticRank, CommitSemanticRank, DecisionSemanticRank

THEOREM ExactLeaderSemanticRankOrderingWellFounded ==
  IsWellFoundedOn(
    ExactLeaderSemanticRankOrdering,
    ExactLeaderSemanticRankCarrier)
BY NatLessThanWellFounded, IsWellFoundedOnSubset,
   WFLexPairOrdering, SMT
   DEF ExactLeaderSemanticRankOrdering,
       ExactLeaderSemanticRankCarrier

THEOREM ExactLeaderSemanticRankOrderingMatchesLess ==
  \A left, right \in ExactLeaderSemanticRankCarrier:
    (<<left, right>> \in ExactLeaderSemanticRankOrdering)
      <=> SemanticRankLess(left, right)
BY SMT
   DEF ExactLeaderSemanticRankOrdering,
       ExactLeaderSemanticRankCarrier,
       SemanticRankLess, LexPairOrdering, OpToRel

AdequateLeaderSemanticCompositionProperty(specification) ==
  specification
    => /\ \A mode \in AdequateLeaderCompositionModes:
             AdequateLeaderModeSource(mode)
               ~> (AdequateLeaderModeGoal(mode)
                    \/ AdequateLeaderProductivePhysicalResidual
                    \/ \E rank \in ExactLeaderSemanticRankCarrier:
                         AdequateLeaderModeRankFrontier(mode, rank))
       /\ \A mode \in AdequateLeaderCompositionModes,
             packet \in AsyncPacketSet,
             leaderContext \in ContextRecords,
             witness \in ValidatorIds,
             roundView \in Views,
             subject \in SubjectOrNone:
             (/\ AdequateLeaderModeActive(mode)
              /\ leaderContext = context
              /\ LeaderWireExactSemanticIdentity(
                   packet.item, leaderContext, witness,
                   roundView, subject)
              /\ LeaderWireTransportHandoff(packet))
               ~> (AdequateLeaderModeGoal(mode)
                    \/ \E rank \in ExactLeaderSemanticRankCarrier:
                         AdequateLeaderModeRankFrontier(mode, rank))
       /\ \A mode \in AdequateLeaderCompositionModes,
             item \in AsyncNetworkItems,
             leaderContext \in ContextRecords,
             witness \in ValidatorIds,
             roundView \in Views,
             subject \in SubjectOrNone:
             (/\ AdequateLeaderModeActive(mode)
              /\ leaderContext = context
              /\ LeaderWireExactSemanticIdentity(
                   item, leaderContext, witness, roundView, subject)
              /\ LeaderWireRunnerAdmissionHandoff(item))
               ~> (AdequateLeaderModeGoal(mode)
                    \/ \E rank \in ExactLeaderSemanticRankCarrier:
                         AdequateLeaderModeRankFrontier(mode, rank))
       /\ \A mode \in AdequateLeaderCompositionModes,
             item \in AsyncNetworkItems,
             leaderContext \in ContextRecords,
             witness \in ValidatorIds,
             roundView \in Views,
             subject \in SubjectOrNone:
             (/\ AdequateLeaderModeActive(mode)
              /\ leaderContext = context
              /\ LeaderWireExactSemanticIdentity(
                   item, leaderContext, witness, roundView, subject)
              /\ item.kind = "CertifiedResponse"
              /\ CertifiedResponsePhysicalCompletionHandoff(item))
               ~> (AdequateLeaderModeGoal(mode)
                    \/ \E rank \in ExactLeaderSemanticRankCarrier:
                         AdequateLeaderModeRankFrontier(mode, rank))
       /\ \A mode \in AdequateLeaderCompositionModes,
             node \in ValidatorIds,
             roundView \in Views:
             (/\ AdequateLeaderModeActive(mode)
              /\ node \in AsyncCurrentResponsiveVoters
              /\ TimeoutQuorumViewRotationHandoff(node, roundView))
               ~> (AdequateLeaderModeGoal(mode)
                    \/ \E rank \in ExactLeaderSemanticRankCarrier:
                         AdequateLeaderModeRankFrontier(mode, rank))
       /\ \A mode \in AdequateLeaderCompositionModes,
             rank \in ExactLeaderSemanticRankCarrier:
            AdequateLeaderModeRankFrontier(mode, rank)
              ~> (AdequateLeaderModeGoal(mode)
                   \/ \E lowerRank \in
                          SetLessThan(
                            rank,
                            ExactLeaderSemanticRankOrdering,
                            ExactLeaderSemanticRankCarrier):
                        AdequateLeaderModeRankFrontier(
                          mode, lowerRank))

\* TODO: prove the semantic-composition property from exact timeout/view
\* rotation, leader selection, and command-successor phase mappings.

THEOREM ExactCandidateHandoffsLowerSemanticRank ==
  \A initialContext:
    AdequateLeaderSemanticCompositionProperty(
      AsyncLiveSpecAt(initialContext))
      => (AsyncLiveSpecAt(initialContext)
            => \A mode \in AdequateLeaderCompositionModes,
                  rank \in ExactLeaderSemanticRankCarrier:
                 AdequateLeaderModeRankFrontier(mode, rank)
                   ~> (AdequateLeaderModeGoal(mode)
                        \/ \E lowerRank \in
                               SetLessThan(
                                 rank,
                                 ExactLeaderSemanticRankOrdering,
                                 ExactLeaderSemanticRankCarrier):
                             AdequateLeaderModeRankFrontier(
                               mode, lowerRank)))
BY DEF AdequateLeaderSemanticCompositionProperty

THEOREM ExactCandidateSemanticRanksReachModeGoal ==
  \A initialContext:
    (/\ ExactLeaderCandidateSemanticHandoffProperty(
          AsyncLiveSpecAt(initialContext))
     /\ AdequateLeaderSemanticCompositionProperty(
          AsyncLiveSpecAt(initialContext)))
      => (AsyncLiveSpecAt(initialContext)
            => \A mode \in AdequateLeaderCompositionModes,
                  rank \in ExactLeaderSemanticRankCarrier:
                 AdequateLeaderModeRankFrontier(mode, rank)
                   ~> AdequateLeaderModeGoal(mode))
BY ExactCandidateHandoffsLowerSemanticRank,
   ExactLeaderSemanticRankOrderingWellFounded,
   WellFoundedLeadsTo

THEOREM ExactPhysicalResidualsReachRankOrModeGoal ==
  \A initialContext:
    (/\ AdequateLeaderExactPhysicalResidualConvergenceProperty(
          AsyncLiveSpecAt(initialContext))
     /\ AdequateLeaderSemanticCompositionProperty(
          AsyncLiveSpecAt(initialContext)))
      => (AsyncLiveSpecAt(initialContext)
            => \A mode \in AdequateLeaderCompositionModes:
                 (gst /\ AdequateLeaderProductivePhysicalResidual)
                   ~> (AdequateLeaderModeGoal(mode)
                        \/ \E rank \in ExactLeaderSemanticRankCarrier:
                             AdequateLeaderModeRankFrontier(mode, rank)))
BY AsyncLiveSpecProjectsAsyncSpec,
   AsyncSpecKeepsGstOnceSet, Isa, PTL
   DEF AdequateLeaderExactPhysicalResidualConvergenceProperty,
       AdequateLeaderSemanticCompositionProperty,
       AdequateLeaderProductivePhysicalResidual,
       AdequateLeaderModeGoal,
       AdequateLeaderModeActive,
       LeaderWireCurrentContextWitnessIdentity,
       LeaderWireTransportHandoff,
       LeaderWireRunnerAdmissionHandoff,
       CertifiedResponsePhysicalCompletionHandoff,
       TimeoutQuorumViewRotationHandoff

THEOREM ExactAdequateLeaderSubkernelsReduceToServiceKernel ==
  \A initialContext:
    /\ AdequateLeaderExactResidualKernelProperty(
         AsyncLiveSpecAt(initialContext))
    /\ AdequateLeaderSemanticCompositionProperty(
         AsyncLiveSpecAt(initialContext))
    => AdequateLeaderServiceKernelProperty(
         AsyncLiveSpecAt(initialContext))
BY ExactResidualKernelSuppliesCandidateSemanticHandoffs,
   ExactResidualKernelSuppliesExactPhysicalConvergence,
   ExactCandidateSemanticRanksReachModeGoal,
   ExactPhysicalResidualsReachRankOrModeGoal,
   AsyncLiveSpecProjectsAsyncSpec,
   AsyncSpecKeepsGstOnceSet, PTL
   DEF AdequateLeaderExactResidualKernelProperty,
       AdequateLeaderSemanticCompositionProperty,
       AdequateLeaderCompositionModes,
       AdequateLeaderModeSource, AdequateLeaderModeGoal,
       AdequateLeaderModeRankFrontier,
       AdequateLeaderProductivePhysicalResidual,
       ExactLeaderSemanticRankCarrier,
       AdequateLeaderServiceKernelProperty

=============================================================================
