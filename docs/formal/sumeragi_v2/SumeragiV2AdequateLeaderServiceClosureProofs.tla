---- MODULE SumeragiV2AdequateLeaderServiceClosureProofs ----
EXTENDS SumeragiV2RotatingLeaderProgressProofs,
        SumeragiV2AsyncCandidateProducerContinuationProofs

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
below that release target.  The physical transport/runner/timeout residual,
certified-response capacity arm, and scheduler-origin readiness are discharged
here.  The semantic-composition property stays explicit and conditional.
***************************************************************************)

(***************************************************************************
The exact semantic-rank and replay-safe Proposal operators are imported
through the lower locked-body leaf from
`SumeragiV2ExactLeaderSemanticRanks`.  Keeping the common vocabulary below
both temporal leaves prevents a backwards Adequate/Rotating dependency.
***************************************************************************)

THEOREM SameRoundDifferentSubjectProposalWithoutHighIsUnsafeForLock ==
  \A node, proposal:
    /\ lockRank[node] # NoRank
    /\ proposal.view = lockRank[node]
    /\ proposal.subject # lockSubject[node]
    /\ proposal.highestPrepareQc = NoPrepareQC
    => ~DurableProposalSafeForLock(node, proposal)
BY Isa DEF DurableProposalSafeForLock

THEOREM UnsafeProposalSignIsNotAnExactLeaderProposalRank ==
  \A candidate, rank:
    /\ candidate.kind = "SignProposal"
    /\ ~SafeProposalSignIntentAt(candidate)
    => ~ExactLeaderProposalRank(candidate, rank)
BY Isa DEF ExactLeaderProposalRank

THEOREM UnsafeProposalSignIsNotAnExactLeaderCandidateRank ==
  \A candidate, rank:
    /\ candidate.kind = "SignProposal"
    /\ ~SafeProposalSignIntentAt(candidate)
    => ~ExactLeaderCandidateRank(candidate, rank)
BY UnsafeProposalSignIsNotAnExactLeaderProposalRank, Isa
   DEF ExactLeaderCandidateRank, ExactLeaderPhaseRank,
       ExactLeaderViewChangeRank, ExactLeaderPrepareRank,
       ExactLeaderPrepareStaticRank, ExactLeaderPrepareSignRank,
       ExactLeaderCommitRank, ExactLeaderCommitStaticRank,
       ExactLeaderCommitSignRank, ExactLeaderDecisionRank

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
    ProtectedServiceFiniteRunnerEpisodeClosureProperty(
      AsyncSpecAt(initialContext))
      => ExactLeaderRankedCandidateExitProperty(
           AsyncLiveSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext,
                ProtectedServiceFiniteRunnerEpisodeClosureProperty(
                  AsyncSpecAt(initialContext))
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
      => \/ nodeView'[candidate.node] > candidate.consumerView
         \/ /\ nodeView'[candidate.node] = candidate.consumerView
            /\ generation'[candidate.node] >
                 candidate.consumerGeneration
BY ExecutePersistInstallAdvancesCertifiedView,
   InstallAdvancesDeliveryTag, Isa
   DEF ExecutePersistInstall, CandidateConsumerCurrent,
       TimeoutViewGoal

THEOREM ExecutePersistDecisionCreatesExactDecisionMilestone ==
  \A candidate:
    ExecutePersistDecision(candidate) => NodeHasDecision(candidate.node)'
BY Isa
   DEF ExecutePersistDecision, CommandMatches,
       PersistDecision, NodeHasDecision

THEOREM PersistDecisionFramesEveryOtherTargetDecision ==
  \A request:
    \A target \in ValidatorIds:
      /\ TypeInvariant
      /\ PersistDecision(request)
      /\ target # request.node
      => (NodeHasDecision(target)' <=> NodeHasDecision(target))
BY Isa
   DEF PersistDecision, NodeHasDecision, TypeInvariant

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
    /\ AsyncCandidateServiceIdentity(other)
         = AsyncCandidateServiceIdentity(candidate)
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
    /\ AsyncCandidateServiceIdentity(other)
         = AsyncCandidateServiceIdentity(candidate)
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

THEOREM PostGstCurrentConsumerRetirementAdvancesPacemakerCoordinate ==
  \A candidate \in AsyncCandidateSet:
    /\ AsyncStrongTypeInvariant
    /\ gst
    /\ AsyncNext
    /\ CandidateConsumerCurrent(candidate)
    /\ ~CandidateConsumerCurrent(candidate)'
    => \/ nodeView'[candidate.node] > candidate.consumerView
       \/ generation'[candidate.node] >
            candidate.consumerGeneration
BY AsyncNextAdvancesNodeViews,
   InstallAdvancesDeliveryTag, IsaT(180)
   DEF AsyncStrongTypeInvariant, StrongInductiveInvariant,
       Safety, CandidateConsumerCurrent, AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep, RunNode, RunNodeWork,
       ResolveRunNodeCandidateProducerContinuation,
       ReplayRunNodeCandidateProducerContinuation,
       AsyncCandidateProducerContinuationExactLocalReplayStep,
       AsyncCandidateProducerContinuationReplayTargetOnlyTurn,
       AsyncCandidateProducerContinuationExactRuntimeReplayStep,
       SerializedRunnerRuntimeStep, SerializedRuntimeStep,
       FifoRuntimeStep, ExecuteCommand,
       ExecutePersistInstall, PersistInstallTC, vars

THEOREM RankedLeaderOwnerExitIsExecutionDiscardPacemakerOrTerminal ==
  \A candidate, rank:
    /\ AsyncStrongTypeInvariant
    /\ ExactLeaderOwnerExitStep(candidate, rank)
    => \/ TerminalLeaderExit(candidate)
       \/ PacemakerRetiredLeaderOwner(candidate)
       \/ SelectedSuccessfulLeaderExecution(candidate)
       \/ SameConsumerLeaderDiscard(candidate)
BY PostGstCurrentConsumerRetirementAdvancesPacemakerCoordinate, Isa
   DEF ExactLeaderOwnerExitStep, TerminalLeaderExit,
       PacemakerRetiredLeaderOwner,
       SelectedSuccessfulLeaderExecution,
       SameConsumerLeaderDiscard,
       ResponsiveProtectedCandidateOwned,
       ProtectedCandidateOwned, CandidateScheduled,
       AsyncNext, AsyncNonCrashStep, AsyncRunnerStep,
       PostGstRunNode, RunNode, RunNodeWork,
       ResolveRunNodeCandidateProducerContinuation,
       ReplayRunNodeCandidateProducerContinuation,
       AsyncCandidateProducerContinuationExactLocalReplayStep,
       AsyncCandidateProducerContinuationReplayTargetOnlyTurn,
       AsyncCandidateProducerContinuationExactRuntimeReplayStep,
       LocalAdmissionStep, IngressDrainStep,
       SerializedRunnerRuntimeStep, SerializedRuntimeStep,
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
  /\ UNCHANGED up
  /\ UNCHANGED <<asyncCommandQueues,
                 asyncDeferredCompletionQueues,
                 asyncDeferredProgressQueues,
                 asyncDeferredNormalQueues,
                 asyncCausalQueues,
                 asyncOutstandingWork,
                 asyncSentItems,
                 asyncHeldChunks,
                 asyncControlServiceState>>

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

THEOREM ExactLeaderSchedulerReadinessFramePreservesSafeProposalSignIntent ==
  \A candidate:
    ExactLeaderSchedulerReadinessFrame
      => (SafeProposalSignIntentAt(candidate)'
            <=> SafeProposalSignIntentAt(candidate))
BY IsaT(60)
   DEF ExactLeaderSchedulerReadinessFrame,
       SafeProposalSignIntentAt, ProposalSignIntentsAt,
       DurableProposalSafeForLock, vars

THEOREM ExactLeaderSchedulerReadinessFramePreservesProposalRank ==
  \A candidate, rank:
    ExactLeaderSchedulerReadinessFrame
      => (ExactLeaderProposalRank(candidate, rank)'
            <=> ExactLeaderProposalRank(candidate, rank))
BY ExactLeaderSchedulerReadinessFramePreservesSafeProposalSignIntent,
   Isa
   DEF ExactLeaderProposalRank

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
PROOF
  <1>1. ASSUME NEW node, NEW proposal,
                ExactLeaderSchedulerReadinessFrame
         PROVE BeginPrepareReady(node, proposal)'
                 <=> BeginPrepareReady(node, proposal)
    <2>1. ProposalWireValidFor(node, proposal)'
             <=> ProposalWireValidFor(node, proposal)
      BY <1>1,
         ExactLeaderSchedulerReadinessFramePreservesProposalWireValidity
    <2> QED BY <1>1, <2>1, IsaT(120)
         DEF ExactLeaderSchedulerReadinessFrame,
             BeginPrepareReady, PrepareSignerAvailability,
             BodyHeldBy, BodyValidatedBy,
             NodeIdle, PendingNodes, SigningNodes, NodeTimedOut,
             PrepareRequestFor, PrepareVoteFor, PrepareWal, Vote,
             CurrentVoters, CurrentEpoch, vars
  <1> QED BY <1>1

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

THEOREM ExactLeaderSchedulerReadinessFramePreservesBeginLockCommitReady ==
  \A node, qc:
    ExactLeaderSchedulerReadinessFrame
      => (BeginLockCommitReady(node, qc)'
            <=> BeginLockCommitReady(node, qc))
BY IsaT(120)
   DEF ExactLeaderSchedulerReadinessFrame,
       BeginLockCommitReady, CurrentOpenPrepareForCommit,
       NodeTimedOut, BodyHeldBy, BodyValidatedBy,
       NodeIdle, PendingNodes, SigningNodes,
       CurrentVoters, CurrentEpoch, Vote, vars

THEOREM ExactLeaderSchedulerReadinessFramePreservesBeginLockEvidence ==
  \A command, qc:
    ExactLeaderSchedulerReadinessFrame
      => (BeginLockCommandEvidenceMatches(command, qc)'
            <=> BeginLockCommandEvidenceMatches(command, qc))
BY ExactLeaderSchedulerReadinessFramePreservesNetworkItems,
   ExactLeaderSchedulerReadinessFramePreservesCertifiedCapability,
   IsaT(120)
   DEF ExactLeaderSchedulerReadinessFrame,
       BeginLockCommandEvidenceMatches,
       MatchingSentCertifiedRequests,
       FrozenCertifiedResponseBinding,
       FrozenCertifiedRequestRegistration,
       CertifiedResponseAuthenticatedOccurrence, vars

THEOREM ExactLeaderSchedulerReadinessFramePreservesRegularBeginLockCommitReady ==
  \A candidate:
    (/\ ExactLeaderSchedulerReadinessFrame
     /\ candidate.kind = "BeginLockCommit")
      => (RegularCoreCommandReady(candidate)'
            <=> RegularCoreCommandReady(candidate))
PROOF
  <1>1. ASSUME NEW candidate,
                ExactLeaderSchedulerReadinessFrame,
                candidate.kind = "BeginLockCommit"
         PROVE RegularCoreCommandReady(candidate)'
                 <=> RegularCoreCommandReady(candidate)
    <2>1. \A qc:
             BeginLockCommitReady(candidate.node, qc)'
               <=> BeginLockCommitReady(candidate.node, qc)
      BY <1>1,
         ExactLeaderSchedulerReadinessFramePreservesBeginLockCommitReady
    <2>2. \A qc:
             BeginLockCommandEvidenceMatches(candidate, qc)'
               <=> BeginLockCommandEvidenceMatches(candidate, qc)
      BY <1>1,
         ExactLeaderSchedulerReadinessFramePreservesBeginLockEvidence
    <2> QED BY <1>1, <2>1, <2>2, IsaT(120)
         DEF ExactLeaderSchedulerReadinessFrame,
             RegularCoreCommandReady, CommandMatches,
             LockCommitQcValues, ReceivedQcValues, vars
  <1> QED BY <1>1

THEOREM ExactLeaderSchedulerReadinessFramePreservesRegularPersistLockCommitReady ==
  \A candidate:
    (/\ ExactLeaderSchedulerReadinessFrame
     /\ candidate.kind = "PersistLockCommit")
      => (RegularCoreCommandReady(candidate)'
            <=> RegularCoreCommandReady(candidate))
PROOF
  <1>1. ASSUME NEW candidate,
                ExactLeaderSchedulerReadinessFrame,
                candidate.kind = "PersistLockCommit"
         PROVE RegularCoreCommandReady(candidate)'
                 <=> RegularCoreCommandReady(candidate)
    <2>1. \A request:
             PersistLockCommitReady(request)'
               <=> PersistLockCommitReady(request)
      BY <1>1, IsaT(120)
         DEF ExactLeaderSchedulerReadinessFrame,
             PersistLockCommitReady, BodyHeldBy,
             RetainedLockedBodyRecord, vars
    <2> QED BY <1>1, <2>1, IsaT(120)
         DEF ExactLeaderSchedulerReadinessFrame,
             RegularCoreCommandReady, CommandMatches, vars
  <1> QED BY <1>1

THEOREM ExactLeaderSchedulerReadinessFramePreservesFormCommitQCReady ==
  \A node, roundView, subject:
    ExactLeaderSchedulerReadinessFrame
      => (FormCommitQCReady(node, roundView, subject)'
            <=> FormCommitQCReady(node, roundView, subject))
BY IsaT(120)
   DEF ExactLeaderSchedulerReadinessFrame,
       FormCommitQCReady, VoteSignersAt,
       CommitRoundAdmissible, LockedPrepareRound,
       QcWireValid, NodeIdle, PendingNodes, SigningNodes,
       CurrentVoters, CurrentEpoch, QC, vars

THEOREM ExactLeaderSchedulerReadinessFramePreservesRegularFormCommitQCReady ==
  \A candidate:
    (/\ ExactLeaderSchedulerReadinessFrame
     /\ candidate.kind = "FormCommitQC")
      => (RegularCoreCommandReady(candidate)'
            <=> RegularCoreCommandReady(candidate))
PROOF
  <1>1. ASSUME NEW candidate,
                ExactLeaderSchedulerReadinessFrame,
                candidate.kind = "FormCommitQC"
         PROVE RegularCoreCommandReady(candidate)'
                 <=> RegularCoreCommandReady(candidate)
    <2>1. FormCommitQCReady(
             candidate.node, candidate.view, candidate.subject)'
               <=> FormCommitQCReady(
                     candidate.node, candidate.view, candidate.subject)
      BY <1>1,
         ExactLeaderSchedulerReadinessFramePreservesFormCommitQCReady
    <2> QED BY <1>1, <2>1, IsaT(120)
         DEF ExactLeaderSchedulerReadinessFrame,
             RegularCoreCommandReady, vars
  <1> QED BY <1>1

THEOREM ExactLeaderSchedulerReadinessFramePreservesBeginDecisionReady ==
  \A node, qc:
    ExactLeaderSchedulerReadinessFrame
      => (BeginDecisionReady(node, qc)'
            <=> BeginDecisionReady(node, qc))
BY IsaT(120)
   DEF ExactLeaderSchedulerReadinessFrame,
       BeginDecisionReady, NodeIdle, PendingNodes, SigningNodes, vars

THEOREM ExactLeaderSchedulerReadinessFramePreservesRegularBeginDecisionReady ==
  \A candidate:
    (/\ ExactLeaderSchedulerReadinessFrame
     /\ candidate.kind = "BeginDecision")
      => (RegularCoreCommandReady(candidate)'
            <=> RegularCoreCommandReady(candidate))
PROOF
  <1>1. ASSUME NEW candidate,
                ExactLeaderSchedulerReadinessFrame,
                candidate.kind = "BeginDecision"
         PROVE RegularCoreCommandReady(candidate)'
                 <=> RegularCoreCommandReady(candidate)
    <2>1. \A qc:
             BeginDecisionReady(candidate.node, qc)'
               <=> BeginDecisionReady(candidate.node, qc)
      BY <1>1,
         ExactLeaderSchedulerReadinessFramePreservesBeginDecisionReady
    <2> QED BY <1>1, <2>1, IsaT(120)
         DEF ExactLeaderSchedulerReadinessFrame,
             RegularCoreCommandReady, CommandMatches,
             ReceivedQcValues, vars
  <1> QED BY <1>1

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
PROOF
  <1>1. ASSUME NEW candidate,
                ExactLeaderSchedulerReadinessFrame,
                candidate.kind = "PersistTimeout"
         PROVE RegularCoreCommandReady(candidate)'
                 <=> RegularCoreCommandReady(candidate)
    <2>1. \A request:
             PersistTimeoutReady(request)'
               <=> PersistTimeoutReady(request)
      BY <1>1, IsaT(60)
         DEF ExactLeaderSchedulerReadinessFrame,
             PersistTimeoutReady, vars
    <2> QED BY <1>1, <2>1, IsaT(120)
         DEF ExactLeaderSchedulerReadinessFrame,
             RegularCoreCommandReady, CommandMatches, vars
  <1> QED BY <1>1

THEOREM ExactLeaderSchedulerReadinessFramePreservesBeginInstallTCReady ==
  \A node, tc:
    ExactLeaderSchedulerReadinessFrame
      => (BeginInstallTCReady(node, tc)'
            <=> BeginInstallTCReady(node, tc))
BY IsaT(120)
   DEF ExactLeaderSchedulerReadinessFrame,
       BeginInstallTCReady,
       StrictSameRoundTcUpgrade, NodeInstalledTC, TcHighRank,
       PrepareQcRank,
       NodeIdle, PendingNodes, SigningNodes, NoDecisionForNode,
       vars

THEOREM ExactLeaderSchedulerReadinessFramePreservesInstallTcEvidence ==
  \A command, tc:
    ExactLeaderSchedulerReadinessFrame
      => (InstallTcEvidenceMatches(command, tc)'
            <=> InstallTcEvidenceMatches(command, tc))
BY ExactLeaderSchedulerReadinessFramePreservesNetworkItems, IsaT(60)
   DEF ExactLeaderSchedulerReadinessFrame,
       InstallTcEvidenceMatches, vars

THEOREM ExactLeaderSchedulerReadinessFramePreservesRegularBeginInstallTCReady ==
  \A candidate:
    (/\ ExactLeaderSchedulerReadinessFrame
     /\ candidate.kind = "BeginInstallTC")
      => (RegularCoreCommandReady(candidate)'
            <=> RegularCoreCommandReady(candidate))
PROOF
  <1>1. ASSUME NEW candidate,
                ExactLeaderSchedulerReadinessFrame,
                candidate.kind = "BeginInstallTC"
         PROVE RegularCoreCommandReady(candidate)'
                 <=> RegularCoreCommandReady(candidate)
    <2>1. \A tc:
             BeginInstallTCReady(candidate.node, tc)'
               <=> BeginInstallTCReady(candidate.node, tc)
      BY <1>1,
         ExactLeaderSchedulerReadinessFramePreservesBeginInstallTCReady
    <2>2. \A tc:
             InstallTcEvidenceMatches(candidate, tc)'
               <=> InstallTcEvidenceMatches(candidate, tc)
      BY <1>1,
         ExactLeaderSchedulerReadinessFramePreservesInstallTcEvidence
    <2> QED BY <1>1, <2>1, <2>2, IsaT(120)
         DEF ExactLeaderSchedulerReadinessFrame,
             RegularCoreCommandReady, ReceivedTcValues, vars
  <1> QED BY <1>1

THEOREM ExactLeaderSchedulerReadinessFramePreservesInstallCertifiedBodyEffectReady ==
  \A node, roundView, subject:
    ExactLeaderSchedulerReadinessFrame
      => (InstallCertifiedBodyEffectReady(node, roundView, subject)'
            <=> InstallCertifiedBodyEffectReady(node, roundView, subject))
BY IsaT(60)
   DEF ExactLeaderSchedulerReadinessFrame,
       InstallCertifiedBodyEffectReady, BodyHeldBy, vars

THEOREM ExactLeaderSchedulerReadinessFramePreservesRegularFetchCertifiedBodyReady ==
  \A candidate:
    (/\ ExactLeaderSchedulerReadinessFrame
     /\ candidate.kind = "FetchCertifiedBody")
      => (RegularCoreCommandReady(candidate)'
            <=> RegularCoreCommandReady(candidate))
PROOF
  <1>1. ASSUME NEW candidate,
                ExactLeaderSchedulerReadinessFrame,
                candidate.kind = "FetchCertifiedBody"
         PROVE RegularCoreCommandReady(candidate)'
                 <=> RegularCoreCommandReady(candidate)
    <2>1. CertifiedResponseCapabilityAuthorized(candidate.item)'
               <=> CertifiedResponseCapabilityAuthorized(candidate.item)
      BY <1>1,
         ExactLeaderSchedulerReadinessFramePreservesCertifiedCapability
    <2>2. InstallCertifiedBodyEffectReady(
             candidate.node, candidate.view, candidate.subject)'
               <=> InstallCertifiedBodyEffectReady(
                     candidate.node, candidate.view, candidate.subject)
      BY <1>1,
         ExactLeaderSchedulerReadinessFramePreservesInstallCertifiedBodyEffectReady
    <2> QED BY <1>1, <2>1, <2>2, IsaT(120)
         DEF ExactLeaderSchedulerReadinessFrame,
             RegularCoreCommandReady, vars
  <1> QED BY <1>1

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
BY ExactLeaderSchedulerReadinessFramePreservesRegularReady,
   ExactLeaderSchedulerReadinessFramePreservesDecisionFetchReady,
   ExactLeaderSchedulerReadinessFramePreservesSignProposalReady,
   ExactLeaderSchedulerReadinessFramePreservesSignVoteReady,
   ExactLeaderSchedulerReadinessFramePreservesFormPrepareQCReady,
   ExactLeaderSchedulerReadinessFramePreservesSignTimeoutReady,
   ExactLeaderSchedulerReadinessFramePreservesPersistInstallReady,
   ExactLeaderSchedulerReadinessFramePreservesPersistDecisionReady,
   ExactLeaderSchedulerReadinessFramePreservesRequestCertifiedBodyReady,
   ExactLeaderSchedulerReadinessFramePreservesApplyReady,
   ExactLeaderSchedulerReadinessFramePreservesCoreDeliveryReady,
   ExactLeaderSchedulerReadinessFramePreservesChunkDeliveryReady,
   ExactLeaderSchedulerReadinessFramePreservesRejectJunkReady,
   IsaT(120)
   DEF CommandExecutionReady

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
BY ExactLeaderSchedulerReadinessFramePreservesCandidateSet,
   ExactLeaderSchedulerReadinessFramePreservesRank,
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

ExactLeaderControlOccurrenceRetiredByNetwork(candidate) ==
  /\ candidate.item \in AsyncNetworkItems
  /\ candidate.item.kind \in AsyncControlKinds
  /\ AsyncControlServiceOccurrenceIsCurrentOwner(candidate.item)
  /\ ~AsyncControlServiceOccurrenceIsCurrentOwner(candidate.item)'

THEOREM AsyncNetworkReplacementRetiresReadyOccurrenceIntoAuthenticatedProvenance ==
  \A candidate, rank:
    /\ AsyncStrongTypeInvariant
    /\ ExactLeaderCandidateRank(candidate, rank)
    /\ ExecuteCoreDeliveryReady(candidate)
    /\ ExactLeaderControlOccurrenceRetiredByNetwork(candidate)
    /\ AsyncNext
    /\ AsyncNetworkStep
    => AuthenticatedLeaderDiscardProvenance(candidate)'
BY IsaT(300)
   DEF ExactLeaderControlOccurrenceRetiredByNetwork,
       ExactLeaderCandidateRank, ExactLeaderPhaseRank,
       ExactLeaderViewChangeRank, ExactLeaderProposalRank,
       ExactLeaderPrepareRank, ExactLeaderPrepareStaticRank,
       ExactLeaderPrepareSignRank, ExactLeaderCommitRank,
       ExactLeaderCommitStaticRank, ExactLeaderCommitSignRank,
       ExactLeaderDecisionRank,
       CommandExecutionReady, ExecuteCoreDeliveryReady,
       ExecuteChunkDeliveryReady,
       IngressItemHasAuthenticatedHistory,
       AuthenticatedLeaderDiscardProvenance,
       AsyncNext, AsyncNonCrashStep, AsyncNonRunnerStep,
       AsyncNetworkStep, AdmitIngressPacket, AdmitHiddenPacket,
       CoalesceHiddenPacket, DropPolicyRejectedHiddenPacket,
       AsyncControlServiceSlotTransition,
       AsyncControlServiceAdmissionsThisStep,
       AsyncControlServicesThisStep,
       AsyncControlServiceStateAfterAdmission,
       AsyncControlServiceStateAfterService,
       AsyncControlServiceOccurrenceIsCurrentOwner,
       AsyncControlServiceSlotOwned,
       AsyncControlServiceRecordForItem,
       AsyncControlServiceIdentityMatches

THEOREM AsyncNetworkStepPreservesExactLeaderSchedulerOriginProvenance ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncProgressOwnershipInvariant
  /\ ExactLeaderSchedulerOriginProvenanceInvariant
  /\ AsyncNext
  /\ AsyncNetworkStep
  => ExactLeaderSchedulerOriginProvenanceInvariant'
BY AsyncNetworkReplacementRetiresReadyOccurrenceIntoAuthenticatedProvenance,
   IsaT(600)
   DEF ExactLeaderSchedulerOriginProvenanceInvariant,
       DispatchableSameIdentityLeaderOwner,
       ExactLeaderEvidenceAt, ExactLeaderDiscardProvenanceAt,
       AuthenticatedLeaderDiscardProvenance,
       PriorPhaseLeaderDiscardProvenance,
       ExactLeaderSchedulerParked,
       ExactLeaderCandidateRank, ExactLeaderPhaseRank,
       ExactLeaderViewChangeRank, ExactLeaderProposalRank,
       ExactLeaderPrepareRank, ExactLeaderPrepareStaticRank,
       ExactLeaderPrepareSignRank, ExactLeaderCommitRank,
       ExactLeaderCommitStaticRank, ExactLeaderCommitSignRank,
       ExactLeaderDecisionRank,
       CommandExecutionReady, CommandDispatchable,
       ExecuteCoreDeliveryReady, ExecuteChunkDeliveryReady,
       AsyncNext, AsyncNonCrashStep, AsyncNonRunnerStep,
       AsyncNetworkStep, AdmitIngressPacket, AdmitHiddenPacket,
       CoalesceHiddenPacket, DropPolicyRejectedHiddenPacket,
       AsyncControlServiceSlotTransition,
       AsyncControlServiceAdmissionsThisStep,
       AsyncControlServicesThisStep,
       AsyncControlServiceOccurrenceIsCurrentOwner,
       CandidateScheduled, CandidateScheduledIn,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, SequenceSet

THEOREM AsyncNetworkStepPreservesExactLeaderSchedulerOriginReadiness ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncProgressOwnershipInvariant
  /\ ExactLeaderSchedulerOriginReadinessInvariant
  /\ AsyncNext
  /\ AsyncNetworkStep
  => ExactLeaderSchedulerOriginReadinessInvariant'
BY AsyncNetworkStepPreservesExactLeaderSchedulerOriginProvenance,
   AsyncNextPreservesStrongTypeInvariant,
   ExactLeaderOriginProvenanceImpliesIdleReadiness, Isa
   DEF ExactLeaderSchedulerOriginReadinessInvariant

(***************************************************************************
Full source-action induction for scheduler origin.

The induction context contains only independently proved safety invariants.
It does not add fairness or a temporal progress premise.  Each action family
below either transfers an already scheduled immutable candidate, admits an
authenticated ingress occurrence, creates one of the closed causal/restart
constructor inventories, or removes an owner after leaving its exact witness.
The final `AsyncNext` theorem is therefore an action-safety result and does not
call rotating-leader Decision convergence.
***************************************************************************)

ExactLeaderSchedulerOriginInductionSafetyContext ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncProgressOwnershipInvariant
  /\ DecisionFrontierUniquenessInvariant
  /\ DecisionTimeoutFrontierInvariant
  /\ ResponsiveRecoveryValidationClearedInvariant
  /\ FinalProgressWitnessClosureInvariant
  /\ ReplayTailCommitReadyInvariant
  /\ AsyncCandidateServiceTombstoneLifecycleInvariant

ExactLeaderSchedulerOriginInductionContext ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncProgressOwnershipInvariant
  /\ DecisionFrontierUniquenessInvariant
  /\ DecisionTimeoutFrontierInvariant
  /\ ResponsiveRecoveryValidationClearedInvariant
  /\ FinalProgressWitnessClosureInvariant
  /\ ReplayTailCommitReadyInvariant
  /\ AsyncCandidateServiceTombstoneLifecycleInvariant
  /\ ExactLeaderSchedulerOriginReadinessInvariant

THEOREM AsyncLocalAdmissionPreservesExactLeaderSchedulerOriginProvenance ==
  \A node \in ValidatorIds:
    /\ ExactLeaderSchedulerOriginInductionContext
    /\ AsyncNext
    /\ LocalAdmissionStep(node)
    => ExactLeaderSchedulerOriginProvenanceInvariant'
BY AsyncCandidateCausalAdmissionTransfersSameOwner,
   AsyncCandidateProducerCompletionTransfersSameOwner,
   IsaT(600)
   DEF ExactLeaderSchedulerOriginInductionContext,
       ExactLeaderSchedulerOriginProvenanceInvariant,
       LocalAdmissionStep, LocalAdmissionCanAdvance,
       SelectedLocalSource, PreferredLocalSource,
       AdmitCausalHead, CausalHeadCanAdvance, CandidateInFlight,
       AdmitProducerCompletion, ProducerCompletionCanAdvance,
       SelectedCompletionCandidate, EnqueueCandidate,
       UpdateLocalAdmissionMetadata, RecordBlockedCausalDebt,
       DispatchableSameIdentityLeaderOwner,
       ExactLeaderEvidenceAt, ExactLeaderDiscardProvenanceAt,
       ExactLeaderSchedulerParked,
       ExactLeaderCandidateRank, CommandExecutionReady,
       CommandDispatchable, CandidateScheduled,
       CandidateScheduledIn, QueuedCandidates, DeferredCandidates,
       CausalCandidates, TrackedWorkCandidates, SequenceSet,
       AsyncNext, AsyncControlServiceSlotTransition

THEOREM AsyncSelectedLocalAdmissionPreservesExactLeaderSchedulerOriginProvenance ==
  \A node \in ValidatorIds:
    /\ ExactLeaderSchedulerOriginInductionContext
    /\ AsyncNext
    /\ SelectedLocalAdmissionAdvance(node)
    => ExactLeaderSchedulerOriginProvenanceInvariant'
BY AsyncCandidateCausalAdmissionTransfersSameOwner,
   AsyncCandidateProducerCompletionTransfersSameOwner,
   IsaT(600)
   DEF ExactLeaderSchedulerOriginInductionContext,
       ExactLeaderSchedulerOriginProvenanceInvariant,
       SelectedLocalAdmissionAdvance, LocalAdmissionCanAdvance,
       SelectedLocalSource, PreferredLocalSource,
       AdmitCausalHead, CausalHeadCanAdvance, CandidateInFlight,
       AdmitProducerCompletion, ProducerCompletionCanAdvance,
       SelectedCompletionCandidate, EnqueueCandidate,
       UpdateLocalAdmissionMetadata,
       DispatchableSameIdentityLeaderOwner,
       ExactLeaderEvidenceAt, ExactLeaderDiscardProvenanceAt,
       ExactLeaderSchedulerParked,
       ExactLeaderCandidateRank, CommandExecutionReady,
       CommandDispatchable, CandidateScheduled,
       CandidateScheduledIn, QueuedCandidates, DeferredCandidates,
       CausalCandidates, TrackedWorkCandidates, SequenceSet,
       AsyncNext, AsyncControlServiceSlotTransition

THEOREM AsyncIngressDrainPreservesExactLeaderSchedulerOriginProvenance ==
  \A node \in ValidatorIds:
    /\ ExactLeaderSchedulerOriginInductionContext
    /\ AsyncNext
    /\ IngressDrainStep(node)
    => ExactLeaderSchedulerOriginProvenanceInvariant'
BY AsyncActiveControlServiceAdmissionPassesSlotGuard,
   AsyncRetiredControlServiceAdmissionDropsWithoutCandidate,
   IsaT(900)
   DEF ExactLeaderSchedulerOriginInductionContext,
       ExactLeaderSchedulerOriginProvenanceInvariant,
       IngressDrainStep, DrainFairIngressSelected,
       PopSelectedIngress, IngressItemCanDrain,
       DeliveryCandidate, DeliveryKind, DeliveryClass,
       CertifiedResponseCandidate,
       CommitCertificateResponseCandidate,
       CandidateAdmissionCoalesced, EnqueueCandidate,
       IngressItemHasAuthenticatedHistory,
       AuthenticatedLeaderDiscardProvenance,
       DispatchableSameIdentityLeaderOwner,
       ExactLeaderEvidenceAt, ExactLeaderDiscardProvenanceAt,
       ExactLeaderSchedulerParked,
       ExactLeaderCandidateRank, ExactLeaderPhaseRank,
       CommandExecutionReady, CommandDispatchable,
       ExecuteCoreDeliveryReady, ExecuteChunkDeliveryReady,
       ExecuteRejectAuthenticatedJunkReady,
       CandidateScheduled, CandidateScheduledIn,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, SequenceSet,
       AsyncNext, AsyncControlServiceSlotTransition,
       AsyncControlServicesThisStep

THEOREM AsyncFifoRuntimePreservesExactLeaderSchedulerOriginProvenance ==
  \A node \in ValidatorIds:
    /\ ExactLeaderSchedulerOriginInductionContext
    /\ AsyncNext
    /\ FifoRuntimeStep(node)
    => ExactLeaderSchedulerOriginProvenanceInvariant'
BY FifoSuccessfulLeaderExecutionSchedulesDeclaredSuccessors,
   AsyncCandidateBusyDeferralTransfersSameOwner,
   ExecuteSignProposalCreatesExactWireMilestone,
   ExecuteSignVoteCreatesPhaseExactWireMilestone,
   ExecuteFormPrepareQcCreatesExactWireMilestone,
   ExecuteSignTimeoutCreatesExactWireMilestone,
   ExecutePersistInstallCreatesExactPacemakerMilestone,
   ExecutePersistDecisionCreatesExactDecisionMilestone,
   IsaT(900)
   DEF ExactLeaderSchedulerOriginInductionContext,
       ExactLeaderSchedulerOriginProvenanceInvariant,
       FifoRuntimeStep, RemoveNextNodeCommand, NextNodeCommand,
       ExecuteCommand, ExecuteRegularCommand,
       ExecuteDecisionFetch, ExecuteSignProposal, ExecuteSignVote,
       ExecuteFormPrepareQC, ExecuteSignTimeout,
       ExecutePersistInstall, ExecutePersistDecision,
       ExecuteRequestCertifiedBody, ExecuteApply,
       ExecuteCoreDelivery, ExecuteChunkDelivery,
       ExecuteRejectAuthenticatedJunk,
       AppendCausalSuccessors, FreshCommandSuccessors,
       FreshCandidateSequence, CommandSuccessors,
       CausalCandidate, CausalCandidateWithEvidence,
       RetainedBodyRebindCandidate,
       DeferCommand, DiscardCommand,
       DispatchableSameIdentityLeaderOwner,
       ExactLeaderEvidenceAt, ExactLeaderDiscardProvenanceAt,
       AuthenticatedLeaderDiscardProvenance,
       PriorPhaseLeaderDiscardProvenance,
       ExactLeaderSchedulerParked,
       ExactLeaderCandidateRank, ExactLeaderPhaseRank,
       CommandExecutionReady, CommandDispatchable,
       CandidateScheduled, CandidateScheduledIn,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, SequenceSet,
       AsyncNext, AsyncControlServiceSlotTransition,
       AsyncControlServicesThisStep,
       AsyncCandidateServicesThisStep,
       AsyncCandidateTerminalDiscardsThisStep

THEOREM AsyncDeferredRuntimePreservesExactLeaderSchedulerOriginProvenance ==
  \A node \in ValidatorIds:
    /\ ExactLeaderSchedulerOriginInductionContext
    /\ AsyncNext
    /\ DeferredDrainStep(node)
    => ExactLeaderSchedulerOriginProvenanceInvariant'
BY DeferredSuccessfulLeaderExecutionSchedulesDeclaredSuccessors,
   AsyncCandidateDeferredHandoffRetainsSameOwner,
   ExecuteSignProposalCreatesExactWireMilestone,
   ExecuteSignVoteCreatesPhaseExactWireMilestone,
   ExecuteFormPrepareQcCreatesExactWireMilestone,
   ExecuteSignTimeoutCreatesExactWireMilestone,
   ExecutePersistInstallCreatesExactPacemakerMilestone,
   ExecutePersistDecisionCreatesExactDecisionMilestone,
   IsaT(900)
   DEF ExactLeaderSchedulerOriginInductionContext,
       ExactLeaderSchedulerOriginProvenanceInvariant,
       DeferredDrainStep, DeferredWorkServiceable,
       DeferredQueueNonempty, NextDeferredCommand,
       RemoveNextDeferredCommand, DeferredClassQueue,
       DeferredHandoffAllowsExecution,
       DeferredHandoffBlocksExecution,
       InstallDeferredHandoff, ClearDeferredHandoff,
       RetainDeferredHandoffs, AdvanceNextDeferredClass,
       ExecuteCommand, ExecuteRegularCommand,
       ExecuteDecisionFetch, ExecuteSignProposal, ExecuteSignVote,
       ExecuteFormPrepareQC, ExecuteSignTimeout,
       ExecutePersistInstall, ExecutePersistDecision,
       ExecuteRequestCertifiedBody, ExecuteApply,
       ExecuteCoreDelivery, ExecuteChunkDelivery,
       ExecuteRejectAuthenticatedJunk,
       AppendCausalSuccessors, FreshCommandSuccessors,
       FreshCandidateSequence, CommandSuccessors,
       CausalCandidate, CausalCandidateWithEvidence,
       RetainedBodyRebindCandidate,
       DiscardCommand,
       DispatchableSameIdentityLeaderOwner,
       ExactLeaderEvidenceAt, ExactLeaderDiscardProvenanceAt,
       AuthenticatedLeaderDiscardProvenance,
       PriorPhaseLeaderDiscardProvenance,
       ExactLeaderSchedulerParked,
       ExactLeaderCandidateRank, ExactLeaderPhaseRank,
       CommandExecutionReady, CommandDispatchable,
       CandidateScheduled, CandidateScheduledIn,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, SequenceSet,
       AsyncNext, AsyncControlServiceSlotTransition,
       AsyncControlServicesThisStep,
       AsyncCandidateServicesThisStep,
       AsyncCandidateTerminalDiscardsThisStep

THEOREM AsyncTimerAndRetransmitProducerPreservesExactLeaderSchedulerOriginProvenance ==
  \A node \in ValidatorIds:
    /\ ExactLeaderSchedulerOriginInductionContext
    /\ AsyncNext
    /\ (DirectTimeoutStep(node)
          \/ DeferredTimeoutStep(node)
          \/ DirectRetransmitStep(node)
          \/ DeferredRetransmitStep(node))
    => ExactLeaderSchedulerOriginProvenanceInvariant'
BY IsaT(600)
   DEF ExactLeaderSchedulerOriginInductionContext,
       ExactLeaderSchedulerOriginProvenanceInvariant,
       DirectTimeoutStep, DeferredTimeoutStep,
       DirectRetransmitStep, DeferredRetransmitStep,
       TimeoutCausalCommand, AppendCausalSuccessors,
       HistoricalLockedRetransmitSuccessors,
       HistoricalLockedRetransmitCandidate,
       AppendHistoricalLockedRetransmitSuccessors,
       FreshCommandSuccessors, FreshCandidateSequence,
       CommandSuccessors, CausalCandidate,
       DispatchableSameIdentityLeaderOwner,
       ExactLeaderEvidenceAt, ExactLeaderDiscardProvenanceAt,
       ExactLeaderSchedulerParked,
       ExactLeaderCandidateRank, ExactLeaderPhaseRank,
       CommandExecutionReady, CommandDispatchable,
       CandidateScheduled, CandidateScheduledIn,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, SequenceSet,
       AsyncNext, AsyncControlServiceSlotTransition

THEOREM AsyncRuntimePreservesExactLeaderSchedulerOriginProvenance ==
  \A node \in ValidatorIds:
    /\ ExactLeaderSchedulerOriginInductionContext
    /\ AsyncNext
    /\ RuntimeStep(node)
    => ExactLeaderSchedulerOriginProvenanceInvariant'
BY AsyncFifoRuntimePreservesExactLeaderSchedulerOriginProvenance,
   AsyncDeferredRuntimePreservesExactLeaderSchedulerOriginProvenance,
   AsyncTimerAndRetransmitProducerPreservesExactLeaderSchedulerOriginProvenance,
   ExactLeaderSchedulerReadinessFramePreservesInvariant,
   IsaT(300)
   DEF ExactLeaderSchedulerOriginInductionContext,
       ExactLeaderSchedulerOriginReadinessInvariant,
       RuntimeStep, DeferredTagStep, DeferredTagExecutable,
       IdleRuntimeStep, ExactLeaderSchedulerReadinessFrame,
       AsyncNext, AsyncNonCrashStep

THEOREM AsyncIoCompletionPreservesExactLeaderSchedulerOriginProvenance ==
  \A node \in ValidatorIds:
    /\ ExactLeaderSchedulerOriginInductionContext
    /\ AsyncNext
    /\ ServiceIoWorkerWork(node)
    => ExactLeaderSchedulerOriginProvenanceInvariant'
BY AsyncCandidateIoCompletionTransfersSameOwner,
   IsaT(600)
   DEF ExactLeaderSchedulerOriginInductionContext,
       ExactLeaderSchedulerOriginProvenanceInvariant,
       ServiceIoWorkerWork, PublishEphemeralItems,
       DispatchableSameIdentityLeaderOwner,
       ExactLeaderEvidenceAt, ExactLeaderDiscardProvenanceAt,
       AuthenticatedLeaderDiscardProvenance,
       ExactLeaderSchedulerParked,
       ExactLeaderCandidateRank, CommandExecutionReady,
       CommandDispatchable, CandidateScheduled,
       CandidateScheduledIn, QueuedCandidates, DeferredCandidates,
       CausalCandidates, TrackedWorkCandidates,
       ConsensusIoCandidates, SequenceSet,
       AsyncNext, AsyncControlServiceSlotTransition

THEOREM AsyncResponsiveRecoveryPreservesExactLeaderSchedulerOriginProvenance ==
  /\ ExactLeaderSchedulerOriginInductionContext
  /\ AsyncNext
  /\ ((\E node \in ValidatorIds: PreGstResponsiveCrash(node))
        \/ PreGstResponsiveRestart
        \/ PreGstResponsiveReplay
        \/ DriveResponsiveReplayHead
        \/ FinishResponsiveReplay
        \/ RearmResponsiveRecovery)
  => ExactLeaderSchedulerOriginProvenanceInvariant'
BY IsaT(900)
   DEF ExactLeaderSchedulerOriginInductionContext,
       ExactLeaderSchedulerOriginProvenanceInvariant,
       PreGstResponsiveCrash, PreGstResponsiveRestart,
       PreGstResponsiveReplay, DriveResponsiveReplayHead,
       FinishResponsiveReplay, RearmResponsiveRecovery,
       RecoveryCoreReplay, ResetNodeSchedulerForRestart,
       RestartReplay, RestartSignatureReplay,
       RestartDecisionReplay, RestartLockedBodyReplay,
       RestartLockedCommitReplay, RestartTimeoutReplay,
       RestartPrepareReplay, RestartProposalReplay,
       RestartRunnerAssembly, FreshRestartCandidateSequence,
       RestartCandidate, ReplayCommitIntentReady,
       DispatchableSameIdentityLeaderOwner,
       ExactLeaderEvidenceAt, ExactLeaderDiscardProvenanceAt,
       ExactLeaderSchedulerParked,
       ExactLeaderCandidateRank, ExactLeaderPhaseRank,
       CommandExecutionReady, CommandDispatchable,
       CandidateScheduled, CandidateScheduledIn,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, SequenceSet,
       AsyncNext, AsyncControlServiceSlotTransition

THEOREM AsyncContinuationReplayPreservesExactLeaderSchedulerOriginProvenance ==
  \A node \in ValidatorIds:
    /\ ExactLeaderSchedulerOriginInductionContext
    /\ AsyncNext
    /\ ReplayRunNodeCandidateProducerContinuation(node)
    => ExactLeaderSchedulerOriginProvenanceInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                ExactLeaderSchedulerOriginInductionContext,
                AsyncNext,
                ReplayRunNodeCandidateProducerContinuation(node)
         PROVE ExactLeaderSchedulerOriginProvenanceInvariant'
    <2>1. CASE
              AsyncCandidateProducerContinuationExactLocalReplayStep(node)
      BY <1>1, <2>1, IsaT(1500)
         DEF ExactLeaderSchedulerOriginInductionContext,
             ExactLeaderSchedulerOriginReadinessInvariant,
             ExactLeaderSchedulerOriginProvenanceInvariant,
             AsyncCandidateProducerContinuationExactLocalReplayStep,
             AsyncCandidateProducerContinuationExactReplayIdentity,
             AsyncCandidateProducerContinuationSelectedLocalCandidate,
             AsyncCandidateProducerContinuationSelectedReplayRecord,
             AsyncCandidateProducerContinuationSelectedResolutionRecord,
             AsyncCandidateProducerContinuationResolutionRequired,
             AsyncCandidateProducerContinuationResolutionReady,
             AsyncCandidateProducerContinuationResolutionRecordsForNode,
             AsyncCandidateProducerContinuationConcreteSuccessorOwned,
             AsyncCandidateProducerContinuationHandoffOwned,
             AsyncCandidateProducerContinuationLocalReplayCarrier,
             AsyncCandidateProducerContinuationSelectedForRunnerResolution,
             AsyncCandidateProducerContinuationRecordAfterStep,
             AsyncSchedulerExceptCausalControlCommandRunnerAndNodeService,
             EnqueueCandidate,
             DispatchableSameIdentityLeaderOwner,
             ExactLeaderEvidenceAt, ExactLeaderDiscardProvenanceAt,
             ExactLeaderSchedulerParked,
             ExactLeaderCandidateRank, ExactLeaderPhaseRank,
             CommandExecutionReady, CommandDispatchable,
             CandidateScheduled, CandidateScheduledAfter,
             CandidateScheduledIn,
             QueuedCandidates, DeferredCandidates, CausalCandidates,
             TrackedWorkCandidates, SequenceSet,
             AsyncNext, AsyncControlServiceSlotTransition
    <2>2. CASE
              AsyncCandidateProducerContinuationReplayTargetOnlyTurn(node)
      BY <1>1, <2>2,
         ExactLeaderSchedulerReadinessFramePreservesInvariant, Isa
         DEF ExactLeaderSchedulerOriginInductionContext,
             ExactLeaderSchedulerOriginReadinessInvariant,
             ExactLeaderSchedulerReadinessFrame,
             AsyncCandidateProducerContinuationReplayTargetOnlyTurn
    <2>3. CASE
              AsyncCandidateProducerContinuationExactRuntimeReplayStep(node)
      <3>1. RuntimeStep(node)
        BY <2>3, Isa
           DEF AsyncCandidateProducerContinuationExactRuntimeReplayStep,
               RuntimeStep
      <3> QED BY <1>1, <3>1,
           AsyncRuntimePreservesExactLeaderSchedulerOriginProvenance
    <2> QED BY <1>1, <2>1, <2>2, <2>3
         DEF ReplayRunNodeCandidateProducerContinuation
  <1> QED BY <1>1

THEOREM AsyncRunNodeWorkPreservesExactLeaderSchedulerOriginProvenance ==
  \A node \in ValidatorIds:
    /\ ExactLeaderSchedulerOriginInductionContext
    /\ AsyncNext
    /\ RunNodeWork(node)
    => ExactLeaderSchedulerOriginProvenanceInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                ExactLeaderSchedulerOriginInductionContext,
                AsyncNext,
                RunNodeWork(node)
         PROVE ExactLeaderSchedulerOriginProvenanceInvariant'
    <2>0. CASE
            ResolveRunNodeCandidateProducerContinuation(node)
      BY <1>1, <2>0, IsaT(1200)
         DEF ExactLeaderSchedulerOriginInductionContext,
             ExactLeaderSchedulerOriginReadinessInvariant,
             ExactLeaderSchedulerOriginProvenanceInvariant,
             ResolveRunNodeCandidateProducerContinuation,
             AsyncSchedulerExceptCausalControlAndNodeService,
             AsyncCandidateProducerContinuationSelectedForRunnerResolution,
             AsyncCandidateProducerContinuationRecordAfterStep,
             AsyncCandidateServiceStateAfterReclamation,
             AsyncControlServiceSlotTransition,
             AsyncNext
    <2>0p. CASE
             ReplayRunNodeCandidateProducerContinuation(node)
      BY <1>1, <2>0p,
         AsyncContinuationReplayPreservesExactLeaderSchedulerOriginProvenance
    <2>1. CASE LocalAdmissionStep(node)
      BY <1>1, <2>1,
         AsyncLocalAdmissionPreservesExactLeaderSchedulerOriginProvenance
    <2>2. CASE IngressDrainStep(node)
      BY <1>1, <2>2,
         AsyncIngressDrainPreservesExactLeaderSchedulerOriginProvenance
    <2>3. CASE SerializedRuntimeStep(node)
                  \/ SerializedRuntimePrecedesServeIngressStep(node)
      BY <1>1, <2>3,
         AsyncRuntimePreservesExactLeaderSchedulerOriginProvenance
         DEF SerializedRuntimeStep,
             SerializedRuntimePrecedesServeIngressStep
    <2>4. CASE AsyncServeIngressTargetOnlyTurn(node)
      BY <1>1, <2>4,
         ExactLeaderSchedulerReadinessFramePreservesInvariant, Isa
         DEF ExactLeaderSchedulerOriginInductionContext,
             ExactLeaderSchedulerOriginReadinessInvariant,
             ExactLeaderSchedulerReadinessFrame,
             AsyncServeIngressTargetOnlyTurn
    <2>5. CASE SerializedLocalPrecedesServeIngressStep(node)
      <3>1. SelectedLocalAdmissionAdvance(node)
        BY <2>5 DEF SerializedLocalPrecedesServeIngressStep
      <3> QED BY <1>1, <3>1,
           AsyncSelectedLocalAdmissionPreservesExactLeaderSchedulerOriginProvenance
    <2> QED BY <1>1, <2>0, <2>0p, <2>1, <2>2, <2>3, <2>4,
                 <2>5
         DEF RunNodeWork
  <1> QED BY <1>1

THEOREM AsyncHistoricalRunnerPreservesExactLeaderSchedulerOriginProvenance ==
  \A node \in ValidatorIds:
    /\ ExactLeaderSchedulerOriginInductionContext
    /\ AsyncNext
    /\ RunHistoricalServer(node)
    => ExactLeaderSchedulerOriginProvenanceInvariant'
BY IsaT(600)
   DEF ExactLeaderSchedulerOriginInductionContext,
       ExactLeaderSchedulerOriginReadinessInvariant,
       ExactLeaderSchedulerOriginProvenanceInvariant,
       RunHistoricalServer, DrainHistoricalIngressSelected,
       HistoricalIdleStep, PopSelectedIngress,
       AsyncServeCachedReplayItems, PublishEphemeralItems,
       DispatchableSameIdentityLeaderOwner,
       ExactLeaderEvidenceAt, ExactLeaderDiscardProvenanceAt,
       AuthenticatedLeaderDiscardProvenance,
       ExactLeaderSchedulerParked,
       ExactLeaderCandidateRank, CommandExecutionReady,
       CommandDispatchable, CandidateScheduled,
       CandidateScheduledIn, QueuedCandidates, DeferredCandidates,
       CausalCandidates, TrackedWorkCandidates, SequenceSet,
       AsyncNext, AsyncControlServiceSlotTransition

THEOREM AsyncFaultPreservesExactLeaderSchedulerOriginProvenance ==
  /\ ExactLeaderSchedulerOriginInductionContext
  /\ AsyncNext
  /\ AsyncFaultStep
  => ExactLeaderSchedulerOriginProvenanceInvariant'
BY IsaT(600)
   DEF ExactLeaderSchedulerOriginInductionContext,
       ExactLeaderSchedulerOriginReadinessInvariant,
       ExactLeaderSchedulerOriginProvenanceInvariant,
       AsyncFaultStep, PreGstLosePacket,
       PreGstServeReceiverCloseRollback, PreGstCrash,
       InjectByzantineNoise, InjectUntrustedTransportCompletion,
       InjectAuthenticatedJunk, InjectByzantineCertifiedRequest,
       AsyncByzantineProposal, AsyncByzantineVote,
       AsyncByzantineTimeout, PublishEphemeralItems,
       DispatchableSameIdentityLeaderOwner,
       ExactLeaderEvidenceAt, ExactLeaderDiscardProvenanceAt,
       AuthenticatedLeaderDiscardProvenance,
       ExactLeaderSchedulerParked,
       ExactLeaderCandidateRank, CommandExecutionReady,
       CommandDispatchable, CandidateScheduled,
       CandidateScheduledIn, QueuedCandidates, DeferredCandidates,
       CausalCandidates, TrackedWorkCandidates, SequenceSet,
       AsyncNext, AsyncControlServiceSlotTransition

THEOREM AsyncNonRunnerPreservesExactLeaderSchedulerOriginProvenance ==
  /\ ExactLeaderSchedulerOriginInductionContext
  /\ AsyncNext
  /\ AsyncNonRunnerStep
  => ExactLeaderSchedulerOriginProvenanceInvariant'
PROOF
  <1>1. ASSUME ExactLeaderSchedulerOriginInductionContext,
              AsyncNext,
              AsyncNonRunnerStep
         PROVE ExactLeaderSchedulerOriginProvenanceInvariant'
    <2>1. CASE AsyncTick
      BY <1>1, <2>1,
         AsyncTickPreservesExactLeaderSchedulerOriginReadiness
         DEF ExactLeaderSchedulerOriginInductionContext,
             ExactLeaderSchedulerOriginReadinessInvariant
    <2>2. CASE \E node \in AsyncArchiveIoServiceNodes:
                  ServiceIoWorker(node)
      BY <1>1, <2>2,
         AsyncIoCompletionPreservesExactLeaderSchedulerOriginProvenance
         DEF ServiceIoWorker
    <2>3. CASE \E node \in asyncHistoricalRecoveryTargets:
                  ServiceHistoricalRecoveryIoWorker(node)
      BY <1>1, <2>3,
         AsyncIoCompletionPreservesExactLeaderSchedulerOriginProvenance,
         HistoricalRecoveryTargetsAreValidators
         DEF ServiceHistoricalRecoveryIoWorker
    <2>4. CASE AsyncNetworkStep
      BY <1>1, <2>4,
         AsyncNetworkStepPreservesExactLeaderSchedulerOriginProvenance
         DEF ExactLeaderSchedulerOriginInductionContext,
             ExactLeaderSchedulerOriginReadinessInvariant
    <2>5. CASE AsyncFaultStep
      BY <1>1, <2>5,
         AsyncFaultPreservesExactLeaderSchedulerOriginProvenance
    <2>6. CASE ~(AsyncTick
                   \/ (\E node \in AsyncArchiveIoServiceNodes:
                         ServiceIoWorker(node))
                   \/ (\E node \in asyncHistoricalRecoveryTargets:
                         ServiceHistoricalRecoveryIoWorker(node))
                   \/ AsyncNetworkStep
                   \/ AsyncFaultStep)
      BY <1>1, <2>6, IsaT(600)
         DEF ExactLeaderSchedulerOriginInductionContext,
             ExactLeaderSchedulerOriginReadinessInvariant,
             ExactLeaderSchedulerOriginProvenanceInvariant,
             AsyncNonRunnerStep, AsyncSetGST,
             OpenHistoricalRecovery,
             DirectCommitCertificateDiscoveryStep,
             DirectHistoricalCommitCertificateDiscoveryStep,
             CommitCertificateDiscoveryStepWork,
             EnqueueIoLocalControl,
             EnqueueHistoricalRecoveryIoLocalControl,
             EnqueueIoLocalControlWork,
             DispatchableSameIdentityLeaderOwner,
             ExactLeaderEvidenceAt, ExactLeaderDiscardProvenanceAt,
             ExactLeaderSchedulerParked,
             ExactLeaderCandidateRank, CommandExecutionReady,
             CommandDispatchable, CandidateScheduled,
             CandidateScheduledIn, QueuedCandidates,
             DeferredCandidates, CausalCandidates,
             TrackedWorkCandidates, SequenceSet,
             AsyncNext, AsyncControlServiceSlotTransition
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4, <2>5, <2>6
  <1> QED BY <1>1

THEOREM AsyncNextPreservesExactLeaderSchedulerOriginProvenance ==
  /\ ExactLeaderSchedulerOriginInductionContext
  /\ AsyncNext
  => ExactLeaderSchedulerOriginProvenanceInvariant'
PROOF
  <1>1. ASSUME ExactLeaderSchedulerOriginInductionContext,
              AsyncNext
         PROVE ExactLeaderSchedulerOriginProvenanceInvariant'
    <2>1. CASE AsyncNonCrashStep
      <3>1. CASE AsyncRunnerStep
        <4>1. CASE \E node \in AsyncCurrentResponsiveVoters:
                      RunNode(node)
          BY <1>1, <2>1, <3>1, <4>1,
             AsyncCurrentResponsiveVotersAreValidators,
             AsyncRunNodeWorkPreservesExactLeaderSchedulerOriginProvenance
             DEF RunNode
        <4>2. CASE \E node \in asyncHistoricalRecoveryTargets:
                      RunHistoricalRecoveryNode(node)
          BY <1>1, <2>1, <3>1, <4>2,
             HistoricalRecoveryTargetsAreValidators,
             AsyncRunNodeWorkPreservesExactLeaderSchedulerOriginProvenance
             DEF RunHistoricalRecoveryNode
        <4>3. CASE \E node \in AsyncResponsiveAppliedArchiveServers:
                      RunHistoricalServer(node)
          BY <1>1, <2>1, <3>1, <4>3,
             AsyncHistoricalRunnerPreservesExactLeaderSchedulerOriginProvenance,
             Isa
             DEF AsyncResponsiveAppliedArchiveServers
        <4> QED BY <3>1, <4>1, <4>2, <4>3 DEF AsyncRunnerStep
      <3>2. CASE AsyncNonRunnerStep
        BY <1>1, <2>1, <3>2,
           AsyncNonRunnerPreservesExactLeaderSchedulerOriginProvenance
      <3>3. CASE DriveResponsiveReplayHead
        BY <1>1, <3>3,
           AsyncResponsiveRecoveryPreservesExactLeaderSchedulerOriginProvenance
      <3>4. CASE FinishResponsiveReplay
        BY <1>1, <3>4,
           AsyncResponsiveRecoveryPreservesExactLeaderSchedulerOriginProvenance
      <3>5. CASE RearmResponsiveRecovery
        BY <1>1, <3>5,
           AsyncResponsiveRecoveryPreservesExactLeaderSchedulerOriginProvenance
      <3> QED BY <2>1, <3>1, <3>2, <3>3, <3>4, <3>5
           DEF AsyncNonCrashStep
    <2>2. CASE \E node \in ValidatorIds: PreGstCrash(node)
      BY <1>1, <2>2,
         AsyncFaultPreservesExactLeaderSchedulerOriginProvenance,
         Isa DEF AsyncFaultStep
    <2>3. CASE \E node \in ValidatorIds:
                  PreGstResponsiveCrash(node)
      BY <1>1, <2>3,
         AsyncResponsiveRecoveryPreservesExactLeaderSchedulerOriginProvenance
    <2>4. CASE PreGstResponsiveRestart
      BY <1>1, <2>4,
         AsyncResponsiveRecoveryPreservesExactLeaderSchedulerOriginProvenance
    <2>5. CASE PreGstResponsiveReplay
      BY <1>1, <2>5,
         AsyncResponsiveRecoveryPreservesExactLeaderSchedulerOriginProvenance
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4, <2>5 DEF AsyncNext
  <1> QED BY <1>1

THEOREM AsyncNextPreservesExactLeaderSchedulerOriginReadiness ==
  /\ ExactLeaderSchedulerOriginInductionContext
  /\ AsyncNext
  => ExactLeaderSchedulerOriginReadinessInvariant'
BY AsyncNextPreservesExactLeaderSchedulerOriginProvenance,
   AsyncNextPreservesStrongTypeInvariant,
   ExactLeaderOriginProvenanceImpliesIdleReadiness, Isa
   DEF ExactLeaderSchedulerOriginReadinessInvariant

THEOREM AsyncAllVarsStutterPreservesExactLeaderSchedulerOriginReadiness ==
  /\ ExactLeaderSchedulerOriginReadinessInvariant
  /\ UNCHANGED AsyncAllVars
  => ExactLeaderSchedulerOriginReadinessInvariant'
BY Isa
   DEF ExactLeaderSchedulerOriginReadinessInvariant,
       ExactLeaderSchedulerOriginProvenanceInvariant,
       ExactLeaderSchedulerIdleReadinessInvariant,
       AsyncAllVars

THEOREM AsyncBracketNextPreservesExactLeaderSchedulerOriginReadiness ==
  /\ ExactLeaderSchedulerOriginInductionContext
  /\ [AsyncNext]_AsyncAllVars
  => ExactLeaderSchedulerOriginReadinessInvariant'
BY AsyncNextPreservesExactLeaderSchedulerOriginReadiness,
   AsyncAllVarsStutterPreservesExactLeaderSchedulerOriginReadiness,
   Isa

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

\* `SameIdentityLeaderOwner` is only a physical ownership continuation.  It
\* is deliberately omitted from this predicate: a caller must drain the
\* finite set of candidates carrying that immutable service identity before
\* it may consume one of these semantic exits.
ExactLeaderCandidateNonContinuationExitOutcome(candidate, rank) ==
  \/ NodeHasDecision(candidate.node)
  \/ ~CandidateConsumerCurrent(candidate)
  \/ ExactLeaderCandidatePostMilestone(candidate, rank)
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
    /\ ProtectedServiceFiniteRunnerEpisodeClosureProperty(
         AsyncSpecAt(initialContext))
    /\ ExactLeaderExitSafetyKernelProperty(
         AsyncLiveSpecAt(initialContext))
      => ExactLeaderCandidateSemanticHandoffProperty(
           AsyncLiveSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext,
                ProtectedServiceFiniteRunnerEpisodeClosureProperty(
                  AsyncSpecAt(initialContext)),
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
  AsyncLeaderWireKinds

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

LeaderWireBoundedControlSlotOwned(item) ==
  /\ item.kind \in AsyncControlKinds
  /\ AsyncControlServiceSlotOwned(item)

LeaderWireLiveControlServiceOwner(item) ==
  /\ item.kind \in AsyncControlKinds
  /\ AsyncControlServiceOccurrenceIsCurrentOwner(item)

LeaderWireTombstonedControlOccurrence(item) ==
  /\ item.kind \in AsyncControlKinds
  /\ AsyncControlServiceOccurrenceTombstoned(item)

LeaderWireStableControlCompletion(item) ==
  /\ item.kind \in AsyncControlKinds
  /\ AsyncControlServiceConsumed(item)

LeaderWireStableCertifiedResponseCompletion(item) ==
  /\ item.kind = "CertifiedResponse"
  /\ \/ BodyHeldBy(durableBodies, item.envelope.recipient, context,
                   item.envelope.view, item.envelope.subject)
     \/ nodeView[item.envelope.recipient] > item.envelope.view
     \/ NodeHasDecision(item.envelope.recipient)

LeaderWireStableCompletionRecorded(item) ==
  CASE item.kind \in AsyncControlKinds ->
         LeaderWireStableControlCompletion(item)
    [] item.kind = "Chunk" ->
         AsyncChunkReceipt(item.envelope.recipient,
                           item.envelope.view,
                           item.envelope.subject,
                           item.envelope.chunk) \in asyncHeldChunks
    [] item.kind = "CertifiedResponse" ->
         LeaderWireStableCertifiedResponseCompletion(item)

\* A terminal record is not another live service owner.  In particular, a
\* control retry whose exact identity has already been consumed (or whose
\* bounded slot has advanced to a strictly newer identity) may still exist in
\* transport while it drains, but it cannot re-enter the logical owner rank.
LeaderWireTerminalLifecycleRecorded(item) ==
  CASE item.kind \in AsyncControlKinds ->
         AsyncControlServiceOccurrenceTombstoned(item)
    [] item.kind = "Chunk" ->
         LeaderWireStableCompletionRecorded(item)
    [] item.kind = "CertifiedResponse" ->
         LeaderWireStableCertifiedResponseCompletion(item)

\* Unlike the physical tombstone above, this marker cannot be established by
\* an unconsumed same-view identity merely winning the shared slot.  It records
\* exact consumption or a strict control high-watermark, and is therefore the
\* carrier for serviced-identity no-resurrection.
LeaderWireServicedLifecycleRecorded(item) ==
  CASE item.kind \in AsyncControlKinds ->
         AsyncControlServiceIdentityServicedOrAdvanced(item)
    [] item.kind = "Chunk" ->
         LeaderWireStableCompletionRecorded(item)
    [] item.kind = "CertifiedResponse" ->
         LeaderWireStableCertifiedResponseCompletion(item)

LeaderWireLogicalServiceActive(item) ==
  ~LeaderWireTerminalLifecycleRecorded(item)

THEOREM LeaderWireUnconsumedControlOwnerIsNotCompletion ==
  \A item:
    LeaderWireLiveControlServiceOwner(item)
      => /\ ~LeaderWireStableCompletionRecorded(item)
         /\ LeaderWireLogicalServiceActive(item)
BY AsyncControlServiceConsumedOccurrenceIsRetired, Isa
   DEF LeaderWireLiveControlServiceOwner,
       LeaderWireStableCompletionRecorded,
       LeaderWireStableControlCompletion,
       LeaderWireTerminalLifecycleRecorded,
       LeaderWireLogicalServiceActive,
       AsyncControlServiceConsumed,
       AsyncControlServiceOccurrenceIsCurrentOwner,
       AsyncControlServiceOccurrenceTombstoned

LeaderWireConsumerMilestone(item) ==
  \/ LeaderWireStableCompletionRecorded(item)
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
  \* Slot ownership is a transport handoff only.  It is deliberately absent
  \* from `LeaderWireRunnerAdmissionHandoff`: an exact live control owner must
  \* still reach its reducer candidate or become an explicitly retired retry.
  /\ \/ LeaderWireIngressOwned(packet.item)
     \/ LeaderWireCandidateOwned(packet.item)
     \/ LeaderWireBoundedControlSlotOwned(packet.item)
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
     \/ LeaderWireTombstonedControlOccurrence(item)
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
    <2>3. CASE AsyncControlServiceAdmissionCoalesced(packet.item)
      <3>1. LeaderWireBoundedControlSlotOwned(packet.item)
        BY <2>3
           DEF AsyncControlServiceAdmissionCoalesced,
               LeaderWireBoundedControlSlotOwned
      <3> QED BY <1>1, <2>1, <3>1
           DEF LeaderWireTransportHandoff
    <2>4. CASE ~AsyncControlServiceAdmissionCoalesced(packet.item)
      <3>1. CertifiedResponsePacketPolicyRejected(packet.item)
        BY <1>1, <2>2, <2>4 DEF IngressPacketPolicyRejected
      <3>2. /\ packet.item.kind = "CertifiedResponse"
             /\ ~CertifiedResponseAuthorized(packet.item)
        BY <3>1 DEF CertifiedResponsePacketPolicyRejected
      <3>3. LeaderCertifiedResponseRelevant(packet.item)
        BY <2>1, <3>2
           DEF LeaderWireCurrentContextWitnessIdentity,
               LeaderWireExactSemanticIdentity, LeaderWireItem
      <3>4. \/ CandidateScheduled(
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
        BY <3>2, <3>3, Isa DEF LeaderCertifiedResponseRelevant
      <3>5. \/ LeaderWireCandidateOwned(packet.item)
             \/ LeaderWireConsumerMilestone(packet.item)
        BY <3>2, <3>4, Isa
           DEF LeaderWireCandidateOwned, LeaderWireConsumerMilestone
      <3> QED BY <1>1, <2>1, <3>5
           DEF LeaderWireTransportHandoff
    <2> QED BY <2>3, <2>4
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

(***************************************************************************
Exact leader-wire physical rank.

The lifecycle prefix uses the immutable receiver-local admission ordinal and
the frozen per-source ingress prefix captured before physical admission.
`ExactDecisionRequestIngressRank` remains the tail: it contributes the
existing mode/capacity/priority/selector/lane/source/runner components.  An
unbound reordered Chunk has no derivable lifecycle record and therefore uses
the zero lifecycle prefix; its finite, coalesced physical episode is bounded
by `AsyncProoflessChunkEpisodeDebtSet`.

The packet component reuses the target-neutral fixed-clock rank.  Atomic Admit
removes the exact due packet in the same transition that installs the Ingress
owner, so the ordinary transport component strictly descends.  Every fair
owner remains individually quantified.
***************************************************************************)
LeaderWirePhysicalLifecycleOrdinalRank(item) ==
  IF /\ AsyncLeaderWireLifecycleExactActive(item)
     /\ LET record == AsyncLeaderWireLifecycleRecordForItem(item)
        IN record.status = "Ingress"
  THEN LET record == AsyncLeaderWireLifecycleRecordForItem(item)
       IN <<Cardinality(
              AsyncLeaderWireEarlierPhysicalOwners(record)),
            Cardinality(
              AsyncLeaderWireFrozenIngressPredecessorDebtSet(record))>>
  ELSE <<0, 0>>

LeaderWirePhysicalLifecycleOrdinalCarrier ==
  Nat \X Nat

LeaderWirePhysicalLifecycleOrdinalOrdering ==
  LexPairOrdering(
    OpToRel(<, Nat), OpToRel(<, Nat), Nat, Nat)

LeaderWirePhysicalIngressDependencyRank(item) ==
  <<LeaderWirePhysicalLifecycleOrdinalRank(item),
    ExactDecisionRequestIngressRank(
      item.envelope.recipient, item)>>

LeaderWirePhysicalIngressDependencyCarrier ==
  LeaderWirePhysicalLifecycleOrdinalCarrier
    \X ExactDecisionRequestIngressRankCarrier

LeaderWirePhysicalIngressDependencyOrdering ==
  LexPairOrdering(
    LeaderWirePhysicalLifecycleOrdinalOrdering,
    ExactDecisionRequestIngressRankOrdering,
    LeaderWirePhysicalLifecycleOrdinalCarrier,
    ExactDecisionRequestIngressRankCarrier)

LeaderWirePhysicalPacketDependencyRank(packet) ==
  ExactDecisionTargetNeutralPacketDependencyRank(packet)

LeaderWirePhysicalLifecycleStageRank(packet, item) ==
  IF LeaderWireRunnerAdmissionHandoff(item)
       \/ LeaderWireTransportResolution(packet)
  THEN 0
  ELSE IF LeaderWireIngressOwned(item)
       THEN 1
       ELSE 2

LeaderWirePhysicalLifecycleStageCarrier == 0..2

LeaderWirePhysicalLifecycleStageOrdering ==
  OpToRel(<, LeaderWirePhysicalLifecycleStageCarrier)

LeaderWirePhysicalDependencyCertificate(packet) ==
  LET item == packet.item
      snapshot ==
        ExactDecisionTargetNeutralFixedClockSnapshot(asyncNow)
  IN [stage |->
        LeaderWirePhysicalLifecycleStageRank(packet, item),
      packetRank |-> LeaderWirePhysicalPacketDependencyRank(packet),
      ingressRank |-> LeaderWirePhysicalIngressDependencyRank(item),
      predecessors |-> snapshot.predecessors,
      producerBudget |->
        ExactDecisionTargetNeutralProducerEpisodeBudget(snapshot)]

AdequateLeaderWirePhysicalConvergenceProperty(specification) ==
  specification
    => /\ \A packet \in AsyncPacketSet:
             (gst
               /\ LeaderWireCurrentContextWitnessIdentity(packet.item)
               /\ packet \in OverdueResponsivePackets)
               ~> (ResponsiveNodesDecide
                    \/ LeaderWireTransportResolution(packet))
       /\ \A packet \in AsyncPacketSet:
             (gst
               /\ LeaderWireProductiveTransportIdentity(packet.item)
               /\ packet \in OverdueResponsivePackets)
               ~> (ResponsiveNodesDecide
                    \/ LeaderWireTransportHandoff(packet))
       /\ \A item \in AsyncNetworkItems:
             (gst
               /\ LeaderWireCurrentContextWitnessIdentity(item)
               /\ LeaderWireIngressOwned(item))
               ~> (ResponsiveNodesDecide
                    \/ LeaderWireRunnerAdmissionHandoff(item))

THEOREM LeaderWirePhysicalLifecycleOrdinalOrderingIsWellFounded ==
  IsWellFoundedOn(
    LeaderWirePhysicalLifecycleOrdinalOrdering,
    LeaderWirePhysicalLifecycleOrdinalCarrier)
BY NatLessThanWellFounded, WFLexPairOrdering
   DEF LeaderWirePhysicalLifecycleOrdinalOrdering,
       LeaderWirePhysicalLifecycleOrdinalCarrier

THEOREM LeaderWirePhysicalIngressDependencyOrderingIsWellFounded ==
  IsWellFoundedOn(
    LeaderWirePhysicalIngressDependencyOrdering,
    LeaderWirePhysicalIngressDependencyCarrier)
BY LeaderWirePhysicalLifecycleOrdinalOrderingIsWellFounded,
   ExactDecisionRequestIngressRankOrderingIsWellFounded,
   WFLexPairOrdering
   DEF LeaderWirePhysicalIngressDependencyOrdering,
       LeaderWirePhysicalIngressDependencyCarrier

THEOREM LeaderWirePhysicalLifecycleOrdinalRankIsInCarrier ==
  \A item:
    /\ AsyncStrongTypeInvariant
    /\ LeaderWireCurrentContextWitnessIdentity(item)
    /\ LeaderWireIngressOwned(item)
    => LeaderWirePhysicalLifecycleOrdinalRank(item)
         \in LeaderWirePhysicalLifecycleOrdinalCarrier
BY FS_Subset, FS_CardinalityType, IsaT(300)
   DEF LeaderWirePhysicalLifecycleOrdinalRank,
       LeaderWirePhysicalLifecycleOrdinalCarrier,
       AsyncLeaderWireEarlierPhysicalOwners,
       AsyncLeaderWireFrozenIngressPredecessorDebtSet,
       AsyncLeaderWireIngressProtectedRecordsAt,
       AsyncLeaderWireLifecycleExactActive,
       AsyncLeaderWireLifecycleRecordForItem,
       AsyncStrongTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncIngressTypeInvariant, AsyncLeaderWireLifecycleTypeInvariant,
       AsyncLeaderWireLifecycleTyped

THEOREM LeaderWirePhysicalIngressDependencyRankIsInCarrier ==
  \A item:
    /\ AsyncStrongTypeInvariant
    /\ LeaderWireCurrentContextWitnessIdentity(item)
    /\ LeaderWireIngressOwned(item)
    => LeaderWirePhysicalIngressDependencyRank(item)
         \in LeaderWirePhysicalIngressDependencyCarrier
BY LeaderWirePhysicalLifecycleOrdinalRankIsInCarrier,
   ExactDecisionRequestIngressPriorityDebtIsNatural,
   ExactDecisionRequestIngressServeCapacityDebtIsNatural,
   CandidateSequenceIndexIsPosition,
   DrainableIngressTurnReachRankIsNatural, IsaT(600)
   DEF LeaderWirePhysicalIngressDependencyRank,
       LeaderWirePhysicalIngressDependencyCarrier,
       ExactDecisionRequestIngressRank,
       ExactDecisionRequestIngressRankCarrier,
       ExactDecisionRequestIngressCapacityRank,
       ExactDecisionRequestIngressReachSelectorRank,
       ExactDecisionRequestIngressSelectorRank,
       ExactDecisionRequestIngressLaneRank,
       ExactDecisionRequestIngressModeRank,
       ExactDecisionRequestIngressTargetServeCapacityDebt,
       ExactDecisionRequestIngressServeCapacityDebt,
       ExactDecisionRequestIngressPriorityDebt,
       ExactDecisionRequestIngressPriorityOwners,
       ExactDecisionRequestIngressLanePosition,
       ExactDecisionRequestIngressLaneIndices,
       ExactDecisionRequestIngressSourcePosition,
       ExactDecisionRequestIngressReachRank,
       ExactDecisionServeLifecycleIdentity,
       AsyncServeLiveReservationOwned, AsyncServeJobQueued,
       IngressSourceServiceRank, IngressResourceSource,
       IngressLane, SequenceSet

THEOREM LeaderWirePacketAdmissionPreservesExactResolution ==
  \A packet \in AsyncPacketSet:
    LET item == packet.item
        recipient == item.envelope.recipient
    IN /\ AsyncStrongTypeInvariant
       /\ AsyncProgressOwnershipInvariant
       /\ AsyncCandidateServiceLifecycleInvariant
       /\ LeaderWireCurrentContextWitnessIdentity(item)
       /\ packet \in OverdueResponsivePackets
       /\ packet = OldestDueSourcePacket(recipient, item.source)
       /\ AdmitIngressPacket(recipient, item.source)
       => /\ LeaderWireTransportResolution(packet)'
          /\ (LeaderWireProductiveTransportIdentity(item)
                => LeaderWireTransportHandoff(packet)')
BY PolicyRejectedLeaderWireAlreadyHasSemanticHandoff,
   AsyncCandidateServiceTombstoneRejectsTransportReadmission,
   AsyncCandidateTerminalIdentityCannotReactivateAtGst,
   IsaT(1200)
   DEF LeaderWireTransportResolution,
       LeaderWireTransportOccurrenceAbsent,
       LeaderWireTransportHandoff,
       LeaderWireCurrentContextWitnessIdentity,
       LeaderWireProductiveTransportIdentity,
       LeaderWireIngressOwned, LeaderWireCandidateOwned,
       LeaderWireBoundedControlSlotOwned,
       LeaderWireConsumerMilestone,
       AdmitIngressPacket, AdmitHiddenPacket,
       CoalesceHiddenPacket, DropPolicyRejectedHiddenPacket,
       DropExactActiveLeaderWireRetry,
       IngressHasCoalescingOwner, IngressPacketPolicyRejected,
       IngressLane, IngressResourceSource, SequenceSet,
       CandidateScheduled, DeliveryCandidate

THEOREM LeaderWireIngressDrainPreservesExactHandoff ==
  \A item \in AsyncNetworkItems:
    LET node == item.envelope.recipient
    IN /\ AsyncStrongTypeInvariant
       /\ AsyncProgressOwnershipInvariant
       /\ AsyncCandidateServiceLifecycleInvariant
       /\ LeaderWireCurrentContextWitnessIdentity(item)
       /\ LeaderWireIngressOwned(item)
       /\ SelectedIngressItemAt(
            node, FirstDrainableIngressIndex(node)) = item
       /\ DrainFairIngressSelected(node)
       => LeaderWireRunnerAdmissionHandoff(item)'
BY AsyncCandidateServiceTombstoneRejectsTransportReadmission,
   AsyncCandidateTerminalIdentityCannotReactivateAtGst,
   IsaT(1800)
   DEF LeaderWireRunnerAdmissionHandoff,
       LeaderWireCurrentContextWitnessIdentity,
       LeaderWireIngressOwned, LeaderWireCandidateOwned,
       LeaderWireTombstonedControlOccurrence,
       LeaderWireConsumerMilestone,
       DrainFairIngressSelected, EnqueueCandidate,
       CandidateAdmissionCoalesced, CandidateScheduled,
       CandidateScheduledIn, DeliveryCandidate,
       CertifiedResponseCandidate,
       IngressLane, IngressResourceSource, SequenceSet

(***************************************************************************
Concrete physical closure.  The fixed-clock packet rank consumes the finite
due prefix and the individually fair atomic admission owners.  The exact
ingress product then consumes the immutable leader-wire predecessor snapshot
before its ordinary mode/capacity/selector/lane/source/runner tail.  Candidate
and Serve replacement use the separate finite producer-ordinal episode.
Proofless Chunk occurrences use their finite/coalesced episode bound and
terminate in the route-neutral receipt tombstone.
***************************************************************************)
THEOREM AsyncSpecProvidesAdequateLeaderWirePhysicalConvergence ==
  \A initialContext:
    AdequateLeaderWirePhysicalConvergenceProperty(
      AsyncSpecAt(initialContext))
BY AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysProgressOwnershipInvariant,
   ExactDecisionAsyncSpecAlwaysCandidateTombstones,
   ExactDecisionTargetNeutralFixedClockOrderingIsWellFounded,
   ExactDecisionTargetNeutralFixedClockDoesNotAddDuePackets,
   ExactDecisionTargetNeutralLaterWorkCannotAcquirePredecessor,
   ExactDecisionTargetNeutralAtomicAdmissionLowersPacketRank,
   ExactDecisionTargetNeutralNonDescentConsumesOrdinal,
   ExactDecisionTargetNeutralFairOwnerUsesAsyncFairness,
   ExactDecisionRequestIngressRankOrderingIsWellFounded,
   LeaderWirePhysicalLifecycleOrdinalOrderingIsWellFounded,
   LeaderWirePhysicalIngressDependencyOrderingIsWellFounded,
   LeaderWirePhysicalLifecycleOrdinalRankIsInCarrier,
   LeaderWirePhysicalIngressDependencyRankIsInCarrier,
   AsyncLeaderWireIngressTicketExcludesLaterLocalWork,
   AsyncSelectedLeaderWirePhysicalCarrierDefinesIngressScheduler,
   AsyncProoflessChunkEpisodeBudgetIsFiniteAndCoalesced,
   AsyncHeldChunkReceiptTombstonesExactProducerEpisode,
   LeaderWirePacketAdmissionPreservesExactResolution,
   LeaderWireIngressDrainPreservesExactHandoff,
   PTL, IsaT(9000)
   DEF AdequateLeaderWirePhysicalConvergenceProperty,
       LeaderWirePhysicalDependencyCertificate,
       LeaderWirePhysicalLifecycleStageRank,
       LeaderWirePhysicalPacketDependencyRank,
       LeaderWirePhysicalIngressDependencyRank,
       LeaderWirePhysicalLifecycleOrdinalRank,
       LeaderWireTransportResolution,
       LeaderWireTransportOccurrenceAbsent,
       LeaderWireTransportHandoff,
       LeaderWireRunnerAdmissionHandoff,
       LeaderWireIngressOwned, LeaderWireCandidateOwned,
       LeaderWireConsumerMilestone,
       AsyncProoflessChunkEpisodeDebtSet,
       AsyncSpecAt, AsyncFairnessAt, AsyncAllVars

THEOREM AsyncLiveProvidesAdequateLeaderWirePhysicalConvergence ==
  \A initialContext:
    AdequateLeaderWirePhysicalConvergenceProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncSpecProvidesAdequateLeaderWirePhysicalConvergence
   DEF AsyncLiveSpecAt

(***************************************************************************
Timeout/view projection into the exact adequate-leader handoff.

The direct timeout proof reaches either Decision or a strictly higher local
view.  In the latter case the timeout ownership invariant retains the exact
certificate which installed that view in `lastInstalledTc`; Core typing puts
the same node/certificate pair in `installedTCs`.  This is a state projection,
not rotating-leader or aggregate-Decision progress.
***************************************************************************)
THEOREM AdvancedResponsiveNodeHasInstalledTimeoutRotationHandoff ==
  \A node \in AsyncCurrentResponsiveVoters,
     roundView \in Views:
    /\ AsyncStrongTypeInvariant
    /\ TimeoutViewOwnershipInvariant
    /\ gst
    /\ nodeView[node] > roundView
    /\ ~NodeHasDecision(node)
    => TimeoutQuorumViewRotationHandoff(node, roundView)
BY IsaT(600)
   DEF TimeoutQuorumViewRotationHandoff,
       TimeoutViewOwnershipInvariant,
       ResponsiveViewCertificateAuthority,
       TimeoutCertificateSemanticIdentity,
       AsyncStrongTypeInvariant, StrongInductiveInvariant,
       Safety, TypeInvariant

AdequateLeaderTimeoutRotationConvergenceProperty(specification) ==
  specification
    => \A node \in ValidatorIds,
          roundView \in Views:
         (gst
           /\ TimeoutQuorumViewRotationResidual(node, roundView))
           ~> (ResponsiveNodesDecide
                \/ TimeoutQuorumViewRotationHandoff(node, roundView))

THEOREM AsyncLiveProvidesAdequateLeaderTimeoutRotationConvergence ==
  \A initialContext:
    AdequateLeaderTimeoutRotationConvergenceProperty(
      AsyncLiveSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE AdequateLeaderTimeoutRotationConvergenceProperty(
                 AsyncLiveSpecAt(initialContext))
    <2>1. TimeoutViewProgressProperty(
             AsyncLiveSpecAt(initialContext))
      BY AsyncLiveProvidesDirectTimeoutViewClosureResidual,
         DirectTimeoutViewDecompositionClosesTimeoutViewProgress
    <2>2. AsyncLiveSpecAt(initialContext)
            => AsyncSpecAt(initialContext)
      BY AsyncLiveSpecProjectsAsyncSpec
    <2>3. AsyncSpecAt(initialContext)
            => []AsyncStrongTypeInvariant
      BY AsyncSpecAlwaysStrongTypeInvariant
    <2>4. AsyncSpecAt(initialContext)
            => []TimeoutViewOwnershipInvariant
      BY TimeoutViewOwnershipInvariantFromAsyncSpec
    <2>5. AsyncSpecAt(initialContext)
            => [](gst => []gst)
      BY AsyncSpecKeepsGstOnceSet
    <2>6. AsyncSpecAt(initialContext)
            => [](AsyncCurrentResponsiveVoters
                   = AsyncVotersAt(initialContext))
      BY AsyncSpecAlwaysUsesFixedResponsiveVoters
    <2>7. ASSUME AsyncLiveSpecAt(initialContext)
           PROVE \A node \in ValidatorIds,
                    roundView \in Views:
                   (gst
                     /\ TimeoutQuorumViewRotationResidual(
                          node, roundView))
                     ~> (ResponsiveNodesDecide
                          \/ TimeoutQuorumViewRotationHandoff(
                               node, roundView))
      <3>1. ASSUME NEW node \in ValidatorIds,
                    NEW roundView \in Views
             PROVE (gst
                     /\ TimeoutQuorumViewRotationResidual(
                          node, roundView))
                     ~> (ResponsiveNodesDecide
                          \/ TimeoutQuorumViewRotationHandoff(
                               node, roundView))
        <4>1. (gst
                /\ TimeoutQuorumViewRotationResidual(node, roundView))
                  ~> (nodeView[node] > roundView
                       \/ NodeHasDecision(node))
          BY <2>1, <2>7, PTL
             DEF TimeoutViewProgressProperty,
                 TimeoutQuorumViewRotationResidual
        <4>2. (gst
                /\ TimeoutQuorumViewRotationResidual(node, roundView))
                  ~> (/\ gst
                       /\ node \in AsyncCurrentResponsiveVoters
                       /\ \/ nodeView[node] > roundView
                          \/ NodeHasDecision(node))
          BY <2>2, <2>5, <2>6, <2>7, <4>1, PTL
             DEF TimeoutQuorumViewRotationResidual
        <4>3. []( /\ gst
                   /\ node \in AsyncCurrentResponsiveVoters
                   /\ \/ nodeView[node] > roundView
                      \/ NodeHasDecision(node)
                  => ResponsiveNodesDecide
                       \/ TimeoutQuorumViewRotationHandoff(
                            node, roundView))
          BY <2>2, <2>3, <2>4, <2>7,
             AdvancedResponsiveNodeHasInstalledTimeoutRotationHandoff,
             PTL
             DEF TimeoutQuorumViewRotationHandoff,
                 ResponsiveNodesDecide
        <4> QED BY <4>2, <4>3, PTL
      <3> QED BY <3>1
    <2> QED BY <2>7
         DEF AdequateLeaderTimeoutRotationConvergenceProperty
  <1> QED BY <1>1

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

THEOREM AsyncLiveProvidesAdequateLeaderOpenPhysicalResidualConvergence ==
  \A initialContext:
    AdequateLeaderOpenPhysicalResidualConvergenceProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncLiveProvidesAdequateLeaderWirePhysicalConvergence,
   AsyncLiveProvidesAdequateLeaderTimeoutRotationConvergence,
   PTL, IsaT(900)
   DEF AdequateLeaderOpenPhysicalResidualConvergenceProperty,
       AdequateLeaderWirePhysicalConvergenceProperty,
       AdequateLeaderTimeoutRotationConvergenceProperty,
       LeaderWireDueTransportResidual,
       LeaderWireRunnerAdmissionResidual

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
    /\ ProtectedServiceFiniteRunnerEpisodeClosureProperty(
         AsyncSpecAt(initialContext))
    /\ AdequateLeaderExactResidualKernelProperty(
         AsyncLiveSpecAt(initialContext))
      => ExactLeaderCandidateSemanticHandoffProperty(
           AsyncLiveSpecAt(initialContext))
BY SchedulerOriginReadinessReducesToExactLeaderExitSafety,
   ExactDiscardSafetyClosesAdmittedCandidateHandoffs
   DEF AdequateLeaderExactResidualKernelProperty

THEOREM AsyncLiveProvidesExactLeaderCandidateSemanticHandoffs ==
  \A initialContext:
    ExactLeaderCandidateSemanticHandoffProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncSpecProvidesProtectedServiceFiniteRunnerEpisodeClosure,
   AsyncLiveExactLeaderSchedulerOriginReadiness,
   SchedulerOriginReadinessReducesToExactLeaderExitSafety,
   ExactDiscardSafetyClosesAdmittedCandidateHandoffs

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

The aggregate frontier immediately below is retained only as diagnostic
vocabulary for the minimum-rank counterexample and the owned action anchor.
It is not used by the release-facing composition at the end of this module.
`PersistDecision` at the minimum realizable rank <<1, 2>> decides only its
selected voter; another responsive voter may retain an equal PersistDecision
or a higher Decision/Commit owner, and the selected voter's recovery
successor may move back to a Proposal/body rank.  The release-facing
composition therefore uses the target-indexed frontier defined after the
finite Decision-prefix proof.

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

\* This operator freezes only the record shape of a rank witness.  Every
\* actual mode frontier below also requires `ExactLeaderCandidateRank`, whose
\* SignProposal arm applies `SafeProposalSignIntentAt`; the static carrier
\* therefore cannot make a superseded replayed ProposalIntent productive.
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

THEOREM PersistDecisionHasMinimumRealizableLeaderSemanticRank ==
  \A candidate, rank:
    /\ candidate.kind = "PersistDecision"
    /\ ExactLeaderPhaseRank(candidate, rank)
    => /\ rank = DecisionSemanticRank(2)
       /\ \A other, otherRank:
            ExactLeaderPhaseRank(other, otherRank)
              => ~SemanticRankLess(otherRank, rank)
BY IsaT(120)
   DEF ExactLeaderPhaseRank,
       ExactLeaderViewChangeRank,
       ExactLeaderProposalRank,
       ExactLeaderPrepareRank,
       ExactLeaderPrepareStaticRank,
       ExactLeaderPrepareSignRank,
       ExactLeaderCommitRank,
       ExactLeaderCommitStaticRank,
       ExactLeaderCommitSignRank,
       ExactLeaderDecisionRank,
       ViewChangeSemanticRank, ProposalSemanticRank,
       PrepareSemanticRank, CommitSemanticRank,
       DecisionSemanticRank, SemanticRankLess

(***************************************************************************
Target-indexed Decision aggregation.

The semantic pipeline must first establish Decision for one frozen responsive
target.  Only after that local theorem is available may the release proof
aggregate over the finite frozen voter roster.  The prefix below performs
exactly that aggregation and uses the action-level durability of each target's
Decision receipt.  It does not assume that a Decision at one validator is a
terminal outcome for any other validator.

`AdequateLeaderTargetDecisionConvergenceProperty` is deliberately a property
declaration.  A target/leader/view/subject-indexed semantic frontier must
discharge it separately; this section proves only the finite composition from
local convergence to the aggregate Decide-mode clause.
***************************************************************************)

AdequateLeaderTargetDecisionSource(target) ==
  /\ gst
  /\ AdequateResponsiveHonestLeaderViewReached
  /\ target \in AsyncCurrentResponsiveVoters
  /\ ~NodeHasDecision(target)

\* Local consumers (notably indexed historical recovery) start at GST before
\* an adequate view is known.  Product-local ownership is explicit: a current
\* responsive voter enters this kernel only after the indexed activation
\* machine has made its reducer serviceable.  Standalone initialization keeps
\* every validator active.  The source assumes neither aggregate Decision nor
\* joined/application state.
AdequateLeaderLocalTargetDecisionSource(target) ==
  /\ gst
  /\ target \in AsyncCurrentResponsiveVoters
  /\ target \in AsyncActiveServiceNodes
  /\ ~NodeHasDecision(target)

AdequateLeaderTargetDecisionConvergenceProperty(specification) ==
  specification
    => \A target \in ValidatorIds:
         AdequateLeaderTargetDecisionSource(target)
           ~> NodeHasDecision(target)

AdequateLeaderLocalTargetDecisionConvergenceProperty(specification) ==
  specification
    => \A target \in ValidatorIds:
         AdequateLeaderLocalTargetDecisionSource(target)
           ~> NodeHasDecision(target)

AdequateLeaderDecisionPrefixAt(initialContext, limit) ==
  \A target \in AsyncVotersAt(initialContext) \cap (0..limit):
    NodeHasDecision(target)

THEOREM AdequateLeaderCoreBracketStepPreservesTargetDecision ==
  \A target:
    NodeHasDecision(target)
      /\ [Next]_vars
      => NodeHasDecision(target)'
PROOF
  <1>1. ASSUME NEW target,
                NodeHasDecision(target),
                [Next]_vars
         PROVE NodeHasDecision(target)'
    <2>1. CASE UNCHANGED vars
      BY <1>1, <2>1, Isa DEF NodeHasDecision, vars
    <2>2. CASE Next
      <3>1. UNCHANGED context
        BY <2>2, CoreNextLeavesContext
      <3>2. \/ UNCHANGED <<decisions, applied>>
             \/ (\E request \in pendingDecision:
                   PersistDecision(request))
             \/ (\E owner \in ValidatorIds,
                        qc \in DecisionQcValues:
                   ApplyDecision(owner, qc))
        BY <2>2, NextDurableReceiptActionClassification
      <3>3. CASE UNCHANGED <<decisions, applied>>
        BY <1>1, <3>1, <3>3, Isa DEF NodeHasDecision
      <3>4. CASE \E request \in pendingDecision:
                    PersistDecision(request)
        <4>1. PICK request \in pendingDecision:
                 PersistDecision(request)
          BY <3>4
        <4> QED BY <1>1, <3>1, <4>1, Isa
             DEF PersistDecision, NodeHasDecision
      <3>5. CASE \E owner \in ValidatorIds,
                        qc \in DecisionQcValues:
                    ApplyDecision(owner, qc)
        <4>1. PICK owner \in ValidatorIds,
                     qc \in DecisionQcValues:
                 ApplyDecision(owner, qc)
          BY <3>5
        <4> QED BY <1>1, <3>1, <4>1, Isa
             DEF ApplyDecision, NodeHasDecision
      <3> QED BY <3>2, <3>3, <3>4, <3>5
    <2> QED BY <1>1, <2>1, <2>2
  <1> QED BY <1>1

THEOREM AdequateLeaderAsyncBracketStepPreservesTargetDecision ==
  \A target:
    NodeHasDecision(target)
      /\ [AsyncNext]_AsyncAllVars
      => NodeHasDecision(target)'
PROOF
  <1>1. ASSUME NEW target,
                NodeHasDecision(target),
                [AsyncNext]_AsyncAllVars
         PROVE NodeHasDecision(target)'
    <2>1. CASE UNCHANGED AsyncAllVars
      BY <1>1, <2>1, Isa
         DEF NodeHasDecision, AsyncAllVars, AsyncSchedulerVars, vars
    <2>2. CASE AsyncNext
      <3>1. [Next]_vars
        BY <2>2, AsyncStepRefinementObligation
      <3> QED BY <1>1, <3>1,
           AdequateLeaderCoreBracketStepPreservesTargetDecision
    <2> QED BY <1>1, <2>1, <2>2
  <1> QED BY <1>1

THEOREM AdequateLeaderDecisionPrefixAtIsStable ==
  \A initialContext:
    \A limit \in Nat:
      AdequateLeaderDecisionPrefixAt(initialContext, limit)
        /\ [AsyncNext]_AsyncAllVars
        => AdequateLeaderDecisionPrefixAt(initialContext, limit)'
BY Isa, AdequateLeaderAsyncBracketStepPreservesTargetDecision
   DEF AdequateLeaderDecisionPrefixAt

THEOREM FrozenContextFullAdequateLeaderDecisionPrefixImpliesResponsiveDecide ==
  \A initialContext:
    /\ ModelConfiguration
    /\ AsyncFrozenContextAt(initialContext)
    /\ AdequateLeaderDecisionPrefixAt(initialContext, N - 1)
    => ResponsiveNodesDecide
BY FrozenContextFixesResponsiveVoters, Isa
   DEF AdequateLeaderDecisionPrefixAt, ResponsiveNodesDecide,
       AsyncVotersAt, ValidatorIds, ModelConfiguration,
       QuorumConfiguration

THEOREM AdequateLeaderTargetConvergenceDecidesFixedFrozenVoter ==
  \A initialContext:
    \A target \in AsyncVotersAt(initialContext):
      /\ AsyncLiveSpecAt(initialContext)
      /\ AdequateLeaderTargetDecisionConvergenceProperty(
           AsyncLiveSpecAt(initialContext))
      => (gst
            /\ AdequateResponsiveHonestLeaderViewReached
            /\ ~ResponsiveNodesDecide)
           ~> NodeHasDecision(target)
PROOF
  <1>1. ASSUME NEW initialContext,
                NEW target \in AsyncVotersAt(initialContext),
                AsyncLiveSpecAt(initialContext),
                AdequateLeaderTargetDecisionConvergenceProperty(
                  AsyncLiveSpecAt(initialContext))
         PROVE (gst
                  /\ AdequateResponsiveHonestLeaderViewReached
                  /\ ~ResponsiveNodesDecide)
                 ~> NodeHasDecision(target)
    <2>1. [](AsyncCurrentResponsiveVoters
               = AsyncVotersAt(initialContext))
      BY <1>1, AsyncLiveSpecProjectsAsyncSpec,
         AsyncSpecAlwaysUsesFixedResponsiveVoters
    <2>2. \A currentTarget \in ValidatorIds:
             AdequateLeaderTargetDecisionSource(currentTarget)
               ~> NodeHasDecision(currentTarget)
      BY <1>1
         DEF AdequateLeaderTargetDecisionConvergenceProperty
    <2>3. AdequateLeaderTargetDecisionSource(target)
             ~> NodeHasDecision(target)
      BY <1>1, <2>1, <2>2, PTL
         DEF AsyncVotersAt, ValidatorIds
    <2>4. []((gst
                /\ AdequateResponsiveHonestLeaderViewReached
                /\ ~ResponsiveNodesDecide)
               => \/ NodeHasDecision(target)
                  \/ AdequateLeaderTargetDecisionSource(target))
      BY <1>1, <2>1, PTL
         DEF AdequateLeaderTargetDecisionSource
    <2> QED BY <2>3, <2>4, PTL
  <1> QED BY <1>1

THEOREM AdequateLeaderTargetConvergenceReachesEveryDecisionPrefix ==
  \A initialContext:
    /\ AsyncLiveSpecAt(initialContext)
    /\ AdequateLeaderTargetDecisionConvergenceProperty(
         AsyncLiveSpecAt(initialContext))
    => \A limit \in Nat:
         (gst
           /\ AdequateResponsiveHonestLeaderViewReached
           /\ ~ResponsiveNodesDecide)
           ~> AdequateLeaderDecisionPrefixAt(initialContext, limit)
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncLiveSpecAt(initialContext),
                AdequateLeaderTargetDecisionConvergenceProperty(
                  AsyncLiveSpecAt(initialContext))
         PROVE \A limit \in Nat:
                 (gst
                   /\ AdequateResponsiveHonestLeaderViewReached
                   /\ ~ResponsiveNodesDecide)
                   ~> AdequateLeaderDecisionPrefixAt(
                        initialContext, limit)
    <2> DEFINE P(limit) ==
           (gst
             /\ AdequateResponsiveHonestLeaderViewReached
             /\ ~ResponsiveNodesDecide)
             ~> AdequateLeaderDecisionPrefixAt(initialContext, limit)
    <2>1. P(0)
      <3>1. CASE 0 \in AsyncVotersAt(initialContext)
        <4>1. (gst
                 /\ AdequateResponsiveHonestLeaderViewReached
                 /\ ~ResponsiveNodesDecide)
                 ~> NodeHasDecision(0)
          BY <1>1, <3>1,
             AdequateLeaderTargetConvergenceDecidesFixedFrozenVoter
        <4> QED BY <4>1, PTL
             DEF P, AdequateLeaderDecisionPrefixAt
      <3>2. CASE 0 \notin AsyncVotersAt(initialContext)
        BY <3>2, PTL DEF P, AdequateLeaderDecisionPrefixAt
      <3> QED BY <3>1, <3>2
    <2>2. ASSUME NEW limit \in Nat,
                  P(limit)
           PROVE P(limit + 1)
      <3>1. CASE limit + 1 \in AsyncVotersAt(initialContext)
        <4>1. (gst
                 /\ AdequateResponsiveHonestLeaderViewReached
                 /\ ~ResponsiveNodesDecide)
                 ~> NodeHasDecision(limit + 1)
          BY <1>1, <3>1,
             AdequateLeaderTargetConvergenceDecidesFixedFrozenVoter
        <4>2. AdequateLeaderDecisionPrefixAt(
                 initialContext, limit)
                 /\ [AsyncNext]_AsyncAllVars
                 => AdequateLeaderDecisionPrefixAt(
                      initialContext, limit)'
          BY <2>2, AdequateLeaderDecisionPrefixAtIsStable
        <4>3. NodeHasDecision(limit + 1)
                 /\ [AsyncNext]_AsyncAllVars
                 => NodeHasDecision(limit + 1)'
          BY AdequateLeaderAsyncBracketStepPreservesTargetDecision
        <4>4. AdequateLeaderDecisionPrefixAt(
                 initialContext, limit + 1)
                 <=> /\ AdequateLeaderDecisionPrefixAt(
                           initialContext, limit)
                     /\ NodeHasDecision(limit + 1)
          BY <2>2, <3>1, Isa
             DEF AdequateLeaderDecisionPrefixAt
        <4> QED BY <2>2, <4>1, <4>2, <4>3, <4>4, PTL DEF P
      <3>2. CASE limit + 1 \notin AsyncVotersAt(initialContext)
        <4>1. AdequateLeaderDecisionPrefixAt(
                 initialContext, limit)
                 => AdequateLeaderDecisionPrefixAt(
                      initialContext, limit + 1)
          BY <2>2, <3>2, Isa
             DEF AdequateLeaderDecisionPrefixAt
        <4> QED BY <2>2, <4>1, PTL DEF P
      <3> QED BY <3>1, <3>2
    <2>3. \A limit \in Nat: P(limit)
      BY <2>1, <2>2, NatInduction
    <2> QED BY <2>3 DEF P
  <1> QED BY <1>1

THEOREM AdequateLeaderTargetDecisionConvergenceSuppliesDecisionMode ==
  \A initialContext:
    AdequateLeaderTargetDecisionConvergenceProperty(
      AsyncLiveSpecAt(initialContext))
      => (AsyncLiveSpecAt(initialContext)
            => (gst
                  /\ AdequateResponsiveHonestLeaderViewReached
                  /\ ~ResponsiveNodesDecide)
                 ~> ResponsiveNodesDecide)
PROOF
  <1>1. ASSUME NEW initialContext,
                AdequateLeaderTargetDecisionConvergenceProperty(
                  AsyncLiveSpecAt(initialContext))
         PROVE AsyncLiveSpecAt(initialContext)
                 => (gst
                       /\ AdequateResponsiveHonestLeaderViewReached
                       /\ ~ResponsiveNodesDecide)
                      ~> ResponsiveNodesDecide
    <2>1. ASSUME AsyncLiveSpecAt(initialContext)
           PROVE (gst
                    /\ AdequateResponsiveHonestLeaderViewReached
                    /\ ~ResponsiveNodesDecide)
                   ~> ResponsiveNodesDecide
      <3>1. ModelConfiguration
        BY <2>1, AsyncLiveSpecProjectsAsyncSpec,
           AsyncSpecAlwaysStrongTypeInvariant, PTL
           DEF AsyncStrongTypeInvariant, StrongInductiveInvariant,
               Safety, TypeInvariant
      <3>2. N - 1 \in Nat
        BY <3>1, SMT DEF ModelConfiguration, QuorumConfiguration
      <3>3. (gst
               /\ AdequateResponsiveHonestLeaderViewReached
               /\ ~ResponsiveNodesDecide)
               ~> AdequateLeaderDecisionPrefixAt(
                    initialContext, N - 1)
        BY <1>1, <2>1, <3>2,
           AdequateLeaderTargetConvergenceReachesEveryDecisionPrefix
      <3>4. []AsyncFrozenContextAt(initialContext)
        BY <2>1, AsyncLiveSpecProjectsAsyncSpec,
           AsyncSpecAlwaysKeepsFrozenContext
      <3>5. [](AdequateLeaderDecisionPrefixAt(
                   initialContext, N - 1)
                 => ResponsiveNodesDecide)
        BY <3>1, <3>4,
           FrozenContextFullAdequateLeaderDecisionPrefixImpliesResponsiveDecide,
           PTL
      <3> QED BY <3>3, <3>5, PTL
    <2> QED BY <2>1
  <1> QED BY <1>1

(***************************************************************************
Target-indexed adequate-leader semantic frontier.

View reach and Decision service are separate temporal modes.  The view-reach
property below is exactly the first adequate-leader service clause.  The
Decision mode freezes one target together with one context, adequate honest
leader, leader view, and protocol subject.  Its semantic rank excludes phase
5: timeout/view rotation belongs to view reach, not to the fixed-view
Decision corridor.

A target frontier may use work owned by the target or by the fixed leader.
Decision commands are stricter: BeginDecision and PersistDecision are
frontier candidates only when their owner is the indexed target.  A
non-target leader's rebroadcasting PersistDecision remains relevant, but as
an explicit producer/transport residual which must expose the CommitQC wire
handoff.  Once that other node has decided, its Fetch/Store/Validate/Apply
recovery continuation is not a target frontier.

The producer/transport residual is deliberately exhaustive.  It names an
exact rebroadcast owner, due transport, runner admission, certified-response
capacity, or the remaining fixed-corridor producer gap.  No theorem below
claims that the implementation discharges those edges.  The only
well-founded reduction is conditional on both their declared closure and the
target-local semantic descent property.
***************************************************************************)

AdequateLeaderTargetSemanticRankCarrier == (1..4) \X (0..9)

AdequateLeaderTargetSemanticRankOrdering ==
  LexPairOrdering(
    OpToRel(<, Nat), OpToRel(<, Nat), 1..4, 0..9)

AdequateLeaderTimeoutLifecycleKinds ==
  AsyncTimeoutLifecycleKinds

AdequateLeaderOlderOrEqualTimeoutLifecycleOwned(
    node, leaderContext, leaderView) ==
  AsyncOlderOrEqualTimeoutLifecycleOwned(
    node, leaderContext, leaderView)

\* The rotating-view prefix spends `AsyncViewSynchronizationBudget` while it
\* installs one exact TC at the frozen responsive roster.  A fixed corridor
\* begins only after that prefix, so it carries the unspent suffix instead of
\* incorrectly demanding the whole synchronization-plus-service budget again.
AdequateLeaderFreshNodeServiceWindow(
    node, leaderContext, leaderView) ==
  /\ node \in ValidatorIds
  /\ leaderContext = context
  /\ leaderView \in Views
  /\ nodeView[node] = leaderView
  /\ asyncNow + AsyncFixedCorridorServiceBudget
       < asyncNodeDeadlines[node]
  /\ ~NodeTimedOut(node, leaderView)
  /\ ~asyncTimeoutEmitted[node]
  /\ "TimeoutElapsed" \notin asyncOutstandingTags[node]
  /\ ~AdequateLeaderOlderOrEqualTimeoutLifecycleOwned(
       node, leaderContext, leaderView)

\* Once the wall clock reaches the deadline, a fixed-view episode may remain
\* protected only by concrete same-node scheduler ownership.  An earlier
\* candidate directly disables both direct and deferred TimeoutElapsed work.
\* An exact ingress record instead owns a smaller recipient-local scheduler
\* ordinal than the current-view BeginTimeout lifecycle.  Its context/view
\* coordinates are checked here; authority for the target Proposal/Chunk is
\* established separately by the corridor-entry refinement.
AdequateLeaderProtectedIngressLifecycleOwned(
    node, leaderContext, leaderView) ==
  /\ AsyncLeaderWireIngressOwnsSharedPhysicalTurn(node)
  /\ LET record ==
           AsyncLeaderWireEarliestPhysicalIngressRecord(node)
     IN /\ record.context = leaderContext
        /\ record.height = leaderContext.height
        /\ record.view = leaderView
        /\ record.schedulerOrdinal
             < AsyncEffectiveTimeoutLifecycleOrdinal(node)

AdequateLeaderProtectedNodeServiceWindow(
    node, leaderContext, leaderView) ==
  /\ node \in ValidatorIds
  /\ leaderContext = context
  /\ leaderView \in Views
  /\ nodeView[node] = leaderView
  /\ AsyncTimeoutClockDue(node)
  /\ ~NodeTimedOut(node, leaderView)
  /\ ~asyncTimeoutEmitted[node]
  /\ "TimeoutElapsed" \notin asyncOutstandingTags[node]
  /\ \/ AsyncOlderCandidateLifecycleBlocksTimeout(node)
     \/ AdequateLeaderProtectedIngressLifecycleOwned(
          node, leaderContext, leaderView)

\* After the fresh synchronization boundary, the fixed episode carries its
\* decreasing service budget separately.  Before expiry it uses the original
\* strict clock arm.  After expiry it survives only through the protected
\* ownership arm above; re-demanding the full fixed budget after every tick or
\* retaining the corridor without a smaller owner would recreate a timing
\* lasso.
AdequateLeaderActiveNodeServiceWindow(
    node, leaderContext, leaderView) ==
  /\ node \in ValidatorIds
  /\ leaderContext = context
  /\ leaderView \in Views
  /\ nodeView[node] = leaderView
  /\ ~NodeTimedOut(node, leaderView)
  /\ ~asyncTimeoutEmitted[node]
  /\ "TimeoutElapsed" \notin asyncOutstandingTags[node]
  /\ \/ /\ asyncNow < asyncNodeDeadlines[node]
         /\ ~AdequateLeaderOlderOrEqualTimeoutLifecycleOwned(
              node, leaderContext, leaderView)
     \/ AdequateLeaderProtectedNodeServiceWindow(
          node, leaderContext, leaderView)

THEOREM AdequateLeaderProtectedCandidateWindowPreventsTimeoutOvertake ==
  \A node \in ValidatorIds,
     leaderContext \in ContextRecords,
     leaderView \in Views:
    /\ AdequateLeaderProtectedNodeServiceWindow(
         node, leaderContext, leaderView)
    /\ AsyncOlderCandidateLifecycleBlocksTimeout(node)
    => /\ ~TimeoutDue(node)
       /\ ~DeferredTimeoutExecutable(node)
BY AsyncOlderCandidateLifecyclePreventsDueTimeoutOvertake
   DEF AdequateLeaderProtectedNodeServiceWindow

THEOREM AdequateLeaderProtectedIngressLifecyclePrecedesTimeout ==
  \A node \in ValidatorIds,
     leaderContext \in ContextRecords,
     leaderView \in Views:
    /\ AsyncLeaderWireLifecycleTypeInvariant
    /\ AdequateLeaderProtectedNodeServiceWindow(
         node, leaderContext, leaderView)
    /\ AdequateLeaderProtectedIngressLifecycleOwned(
         node, leaderContext, leaderView)
    => /\ AsyncLeaderWireIngressProtectedRecordsAt(node) # {}
       /\ AsyncIngressSchedulerBarrierActive(node)
       /\ AsyncEarliestIngressSchedulerOrdinal(node)
            < AsyncEffectiveTimeoutLifecycleOrdinal(node)
BY AsyncSelectedLeaderWirePhysicalCarrierDefinesIngressScheduler, IsaT(120)
   DEF AdequateLeaderProtectedNodeServiceWindow,
       AdequateLeaderProtectedIngressLifecycleOwned

THEOREM AdequateLeaderProtectedIngressWindowPreventsTimeoutOvertake ==
  \A node \in ValidatorIds,
     leaderContext \in ContextRecords,
     leaderView \in Views:
    /\ AsyncLeaderWireLifecycleTypeInvariant
    /\ AdequateLeaderProtectedNodeServiceWindow(
         node, leaderContext, leaderView)
    /\ AdequateLeaderProtectedIngressLifecycleOwned(
         node, leaderContext, leaderView)
    => /\ ~LocalAdmissionStep(node)
       /\ ~SerializedRuntimeStep(node)
       /\ ((/\ AsyncCurrentViewTimeoutLifecycleSelected(node)
             /\ asyncRunnerPhase[node] = "Runtime")
             => /\ ~SerializedRuntimePrecedesServeIngressStep(node)
                /\ (RunNodeWork(node)
                      => \/ ResolveRunNodeCandidateProducerContinuation(
                               node)
                         \/ ReplayRunNodeCandidateProducerContinuation(
                              node)
                         \/ AsyncServeIngressTargetOnlyTurn(node)))
BY AdequateLeaderProtectedIngressLifecyclePrecedesTimeout,
   AsyncLeaderWireIngressTicketExcludesLaterLocalWork,
   AsyncEarlierIngressLifecyclePreventsDueTimeoutOvertake, PTL

AdequateLeaderFrozenResponsiveRoster(leaderContext) ==
  Responsive \cap VotingRoster(leaderContext.epoch)

\* This proof-only receipt is frozen into every candidate/wire owner identity
\* opened by an adequate corridor.  In particular, an exit handoff must carry
\* the same target/context/leader/view authority that admitted the physical
\* episode; it may not reconstruct authority from whichever roster or node
\* view happens to be current after the owner leaves the corridor.
AdequateLeaderCorridorAuthorityReceipt(
    target, leaderContext, leader, leaderView) ==
  [target |-> target,
   context |-> leaderContext,
   leader |-> leader,
   view |-> leaderView,
   roster |-> AdequateLeaderFrozenResponsiveRoster(leaderContext)]

AdequateLeaderCorridorAuthorityReceiptValid(receipt) ==
  /\ receipt.context \in ContextRecords
  /\ receipt.roster =
       AdequateLeaderFrozenResponsiveRoster(receipt.context)
  /\ receipt.roster # {}
  /\ receipt.target \in receipt.roster
  /\ receipt.leader \in receipt.roster \cap Honest
  /\ receipt.view \in Views
  /\ Leader(receipt.context, receipt.view) = receipt.leader
  /\ AsyncViewTimeout(receipt.view) > AsyncWorstCaseServiceBudget

AdequateLeaderResponsiveViewSynchronized(
    leaderContext, leaderView) ==
  /\ AdequateLeaderFrozenResponsiveRoster(leaderContext) # {}
  /\ \A node \in AdequateLeaderFrozenResponsiveRoster(leaderContext):
       AdequateLeaderActiveNodeServiceWindow(
         node, leaderContext, leaderView)

AdequateLeaderFreshTargetLeaderServiceWindow(
    target, leaderContext, leader, leaderView) ==
  /\ AdequateLeaderFreshNodeServiceWindow(
       target, leaderContext, leaderView)
  /\ AdequateLeaderFreshNodeServiceWindow(
       leader, leaderContext, leaderView)

AdequateLeaderActiveTargetLeaderServiceWindow(
    target, leaderContext, leader, leaderView) ==
  /\ AdequateLeaderActiveNodeServiceWindow(
       target, leaderContext, leaderView)
  /\ AdequateLeaderActiveNodeServiceWindow(
       leader, leaderContext, leaderView)

AdequateLeaderFrozenTargetCorridor(
    target, leaderContext, leader, leaderView) ==
  LET authority ==
        AdequateLeaderCorridorAuthorityReceipt(
          target, leaderContext, leader, leaderView)
  IN /\ gst
  /\ AdequateLeaderCorridorAuthorityReceiptValid(authority)
  /\ authority.context = context
  /\ leaderContext \in ContextRecords
  /\ leaderContext = context
  /\ target \in Responsive \cap VotingRoster(leaderContext.epoch)
  /\ leader \in Responsive \cap VotingRoster(leaderContext.epoch)
  /\ leader \in Honest
  /\ leaderView \in Views
  /\ nodeView[leader] = leaderView
  /\ nodeView[target] = leaderView
  /\ Leader(leaderContext, leaderView) = leader
  /\ AsyncViewTimeout(leaderView) > AsyncWorstCaseServiceBudget
  /\ AdequateLeaderResponsiveViewSynchronized(
       leaderContext, leaderView)
  /\ AdequateLeaderActiveTargetLeaderServiceWindow(
       target, leaderContext, leader, leaderView)
  /\ ~NodeHasDecision(target)

AdequateLeaderFreshSynchronizedTargetCorridor(
    target, leaderContext, leader, leaderView) ==
  /\ AdequateLeaderFrozenTargetCorridor(
       target, leaderContext, leader, leaderView)
  /\ \A node \in AdequateLeaderFrozenResponsiveRoster(leaderContext):
       AdequateLeaderFreshNodeServiceWindow(
         node, leaderContext, leaderView)

AdequateLeaderTargetCandidateRole(candidate, target, leader) ==
  /\ candidate.node \in {target, leader}
  /\ IF candidate.kind \in {"BeginDecision", "PersistDecision"}
     THEN candidate.node = target
     ELSE IF candidate.node # target
          THEN ~NodeHasDecision(candidate.node)
          ELSE TRUE

AdequateLeaderFrozenTargetCandidateRole(candidate, target, leader) ==
  /\ candidate.node \in {target, leader}
  /\ (candidate.kind \in {"BeginDecision", "PersistDecision"}
        => candidate.node = target)

\* A fixed-view semantic owner may carry current-view evidence or an older
\* justification, never a future-view object hidden inside its immutable
\* payload.  This is part of exact target-owner classification: a malformed or
\* future-view candidate is scheduler work, but it is not evidence for the
\* frozen adequate-leader corridor and is drained by the outer service kernel.
AdequateLeaderPrepareQcWithinFrozenView(qc, leaderView) ==
  \/ qc = NoPrepareQC
  \/ /\ qc \in QcRecordSet
     /\ qc.view \in 0..leaderView

AdequateLeaderTimeoutVoteWithinFrozenView(vote, leaderView) ==
  /\ vote \in TimeoutVoteRecordSet
  /\ vote.view \in 0..leaderView
  /\ AdequateLeaderPrepareQcWithinFrozenView(
       vote.highestPrepareQc, leaderView)
  /\ vote.highRank \in {NoRank} \cup (0..leaderView)

AdequateLeaderTcWithinFrozenView(tc, leaderView) ==
  \/ tc = NoTimeoutCertificate
  \/ /\ tc \in TcRecordSet
     /\ tc.view \in 0..leaderView
     /\ AdequateLeaderPrepareQcWithinFrozenView(
          tc.highestPrepareQc, leaderView)
     /\ \A vote \in tc.votes:
          AdequateLeaderTimeoutVoteWithinFrozenView(
            vote, leaderView)

AdequateLeaderProposalWithinFrozenView(proposal, leaderView) ==
  /\ proposal \in ProposalRecordSet
  /\ proposal.view \in 0..leaderView
  /\ AdequateLeaderTcWithinFrozenView(
       proposal.timeoutCertificate, leaderView)
  /\ AdequateLeaderPrepareQcWithinFrozenView(
       proposal.highestPrepareQc, leaderView)
  /\ proposal.justifyRank \in {NoRank} \cup (0..leaderView)

AdequateLeaderCertifiedRequestHashWithinFrozenView(
    requestHash, leaderView) ==
  LET signed == requestHash.exactSignedRequest
  IN /\ signed.preimage.round.view \in 0..leaderView
     /\ signed.preimage.certificate \in QcRecordSet
     /\ signed.preimage.certificate.view \in 0..leaderView

AdequateLeaderCandidateItemWithinFrozenView(item, leaderView) ==
  IF item = NoAsyncItem
  THEN TRUE
  ELSE CASE item.kind = "Proposal" ->
              AdequateLeaderProposalWithinFrozenView(
                item.envelope.proposal, leaderView)
         [] item.kind \in {"PrepareVote", "CommitVote"} ->
              item.envelope.vote.view \in 0..leaderView
         [] item.kind \in {"PrepareQC", "CommitQC"} ->
              item.envelope.qc.view \in 0..leaderView
         [] item.kind = "TimeoutVote" ->
              AdequateLeaderTimeoutVoteWithinFrozenView(
                item.envelope.vote, leaderView)
         [] item.kind = "TimeoutCertificate" ->
              AdequateLeaderTcWithinFrozenView(
                item.envelope.tc, leaderView)
         [] item.kind = "CertifiedRequest" ->
              item.envelope.certificate.view \in 0..leaderView
         [] item.kind = "CommitCertificateRequest" ->
              item.envelope.view \in 0..leaderView
         [] item.kind = "CertifiedResponse" ->
              /\ item.envelope.view \in 0..leaderView
              /\ AdequateLeaderCertifiedRequestHashWithinFrozenView(
                   item.envelope.requestHash, leaderView)
         [] item.kind = "CommitCertificateResponse" ->
              /\ item.envelope.request.envelope.view \in 0..leaderView
              /\ item.envelope.qc.view \in 0..leaderView
         [] OTHER -> item.envelope.view \in 0..leaderView

\* The carrier-to-structure direction is needed when exact candidate evidence
\* comes from the record-valued AsyncCandidateSet.  The opposite direction is
\* `TypedItemIsInNetworkCarrier`; after the TC-domain reconciliation the two
\* predicates agree on every configured network item.
THEOREM AsyncNetworkItemCarrierMemberIsTyped ==
  \A item:
    item \in AsyncNetworkItems => AsyncItemTyped(item)
BY IsaT(600)
   DEF AsyncNetworkItems, AsyncItemTyped,
       AsyncTcRecordTyped, AsyncTcEnvelopeTyped,
       AsyncBodyEnvelopeTyped, AsyncReplyRequestItemTyped,
       AsyncCommitCertificateRequestEnvelopeTyped,
       AsyncCertifiedResponseEnvelopeTyped,
       AsyncCommitCertificateResponseEnvelopeTyped,
       AsyncCertifiedRequestItems, AsyncCertifiedRequestHashes,
       AsyncCommitCertificateRequestItems,
       AsyncUntrustedTransportCompletionItem,
       AsyncUntrustedCompletionRequestWitness,
       AsyncUntrustedCompletionQcWitness,
       AsyncCertifiedRequestEnvelope,
       AsyncCertifiedResponseEnvelope,
       AsyncCommitCertificateResponseEnvelope,
       AsyncNetworkItem

AdequateLeaderCandidateEvidenceWithinFrozenView(
    evidence, leaderView) ==
  IF evidence = NoAsyncItem
  THEN TRUE
  ELSE IF AsyncItemTyped(evidence)
       THEN AdequateLeaderCandidateItemWithinFrozenView(
              evidence, leaderView)
       ELSE IF evidence \in ProposalRecordSet
            THEN AdequateLeaderProposalWithinFrozenView(
                   evidence, leaderView)
            ELSE IF evidence \in VoteRecordSet
                 THEN evidence.view \in 0..leaderView
                 ELSE IF evidence \in TimeoutVoteRecordSet
                      THEN AdequateLeaderTimeoutVoteWithinFrozenView(
                             evidence, leaderView)
                      ELSE IF evidence \in QcRecordSet
                           THEN evidence.view \in 0..leaderView
                           ELSE IF evidence \in TcRecordSet
                                THEN AdequateLeaderTcWithinFrozenView(
                                       evidence, leaderView)
                                ELSE /\ evidence \in BodyRecordSet
                                     /\ evidence.view \in 0..leaderView

AdequateLeaderCandidatePayloadWithinFrozenView(candidate, leaderView) ==
  /\ AdequateLeaderCandidateItemWithinFrozenView(
       candidate.item, leaderView)
  /\ AdequateLeaderCandidateEvidenceWithinFrozenView(
       candidate.evidence, leaderView)

AdequateLeaderTargetCandidateIdentity(
    candidate, rank, target, leaderContext, leader, leaderView, subject) ==
  /\ rank \in AdequateLeaderTargetSemanticRankCarrier
  /\ subject \in Subjects
  /\ AdequateLeaderFrozenTargetCorridor(
       target, leaderContext, leader, leaderView)
  /\ ExactLeaderCurrentRankWitness(
       candidate, rank, leaderContext, candidate.node,
       leaderView, subject)
  /\ AdequateLeaderCandidatePayloadWithinFrozenView(
       candidate, leaderView)
  /\ AdequateLeaderFrozenCandidateRootConstructed(
       candidate, target, leaderContext, leader, leaderView)
  /\ AdequateLeaderTargetCandidateRole(candidate, target, leader)

AdequateLeaderFrozenTargetCandidateIdentity(
    candidate, rank, target, leaderContext, leader, leaderView, subject) ==
  /\ rank \in AdequateLeaderTargetSemanticRankCarrier
  /\ subject \in Subjects
  /\ ExactLeaderFrozenSemanticIdentity(
       candidate, rank, leaderContext, candidate.node,
       leaderView, subject)
  /\ AdequateLeaderCandidatePayloadWithinFrozenView(
       candidate, leaderView)
  /\ AdequateLeaderFrozenCandidateRootConstructed(
       candidate, target, leaderContext, leader, leaderView)
  /\ AdequateLeaderFrozenTargetCandidateRole(
       candidate, target, leader)

THEOREM AdequateLeaderTargetCandidateIdentityHasBoundedPayload ==
  \A candidate, rank, target, leaderContext,
     leader, leaderView, subject:
    AdequateLeaderTargetCandidateIdentity(
      candidate, rank, target, leaderContext,
      leader, leaderView, subject)
      => AdequateLeaderCandidatePayloadWithinFrozenView(
           candidate, leaderView)
BY DEF AdequateLeaderTargetCandidateIdentity

THEOREM AdequateLeaderFrozenCandidateIdentityHasBoundedPayload ==
  \A candidate, rank, target, leaderContext,
     leader, leaderView, subject:
    AdequateLeaderFrozenTargetCandidateIdentity(
      candidate, rank, target, leaderContext,
      leader, leaderView, subject)
      => AdequateLeaderCandidatePayloadWithinFrozenView(
           candidate, leaderView)
BY DEF AdequateLeaderFrozenTargetCandidateIdentity

\* Every ordinary causal continuation inside the fixed Decision corridor is
\* strictly later in the protocol pipeline.  PersistDecision is deliberately
\* excluded: a target-owned instance decides, while a leader-owned instance
\* must first pass through the separate exact CommitQC rebroadcast corridor.
\* Treating that continuation as an ordinary lower candidate would silently
\* prove Decision for the wrong validator.
THEOREM AdequateLeaderNonDecisionDeclaredSuccessorStrictlyLowersStaticRank ==
  \A parent, child, parentRank, childRank:
    /\ parentRank \in AdequateLeaderTargetSemanticRankCarrier
    /\ childRank \in AdequateLeaderTargetSemanticRankCarrier
    /\ parent.kind # "PersistDecision"
    /\ ExactLeaderStaticSemanticRank(parent, parentRank)
    /\ child \in SequenceSet(CommandSuccessors(parent))
    /\ ExactLeaderStaticSemanticRank(child, childRank)
    => <<childRank, parentRank>>
         \in AdequateLeaderTargetSemanticRankOrdering
BY IsaT(600)
   DEF ExactLeaderStaticSemanticRank,
       AdequateLeaderTargetSemanticRankCarrier,
       AdequateLeaderTargetSemanticRankOrdering,
       CommandSuccessors, CausalSuccessorParentKinds,
       CausalCandidate, CausalCandidateWithEvidence,
       RetainedBodyRebindCandidate,
       PersistDecisionRecoverySuccessor,
       InstallCommandSuccessors,
       InstallLockedFetchSuccessors,
       InstallCommitSignSuccessors,
       InstallProposalSuccessor,
       SequenceSet,
       ViewChangeSemanticRank, ProposalSemanticRank,
       PrepareSemanticRank, CommitSemanticRank,
       DecisionSemanticRank, LexPairOrdering, OpToRel

AdequateLeaderTargetRankFrontier(
    target, leaderContext, leader, leaderView, subject, rank) ==
  \E candidate \in AsyncCandidateSet:
    AdequateLeaderTargetCandidateIdentity(
      candidate, rank, target, leaderContext, leader, leaderView, subject)

AdequateLeaderTargetRankOwnerSet(
    target, leaderContext, leader, leaderView, subject, rank) ==
  {candidate \in AsyncCandidateSet:
     AdequateLeaderTargetCandidateIdentity(
       candidate, rank, target, leaderContext, leader, leaderView, subject)}

\* Candidate owner identity must distinguish an immutable semantic payload
\* replacement, not merely a work kind.  Consumer generation/view are
\* deliberately absent: they are process-incarnation coordinates and a replay
\* after restart retains the same logical work.  Every protocol view nested
\* inside authenticated evidence is retained up to the frozen leader view; a
\* value above that view maps to one explicit out-of-corridor coordinate.
\* Valid QC signer supersets and valid TC timeout-share/group supersets are
\* deliberately projected to the same certificate reference.  They are
\* replaceable authenticated carriers for one reducer occurrence, not new
\* owners.  Context, view, phase, subject, proposer/signer, selected
\* highest-Prepare reference, work class, and recovery identities remain
\* distinct, yielding a finite semantic range for one frozen corridor.
AdequateLeaderFrozenViewCoordinate(roundView, leaderView) ==
  IF roundView \in 0..leaderView THEN roundView ELSE leaderView + 1

AdequateLeaderFrozenQcPayload(qc, leaderView) ==
  [context |-> qc.context,
   height |-> qc.height,
   view |-> AdequateLeaderFrozenViewCoordinate(qc.view, leaderView),
   phase |-> qc.phase,
   subject |-> qc.subject]

AdequateLeaderFrozenPrepareQcPayload(qc, leaderView) ==
  IF qc = NoPrepareQC
  THEN NoPrepareQC
  ELSE AdequateLeaderFrozenQcPayload(qc, leaderView)

AdequateLeaderFrozenVotePayload(vote, leaderView) ==
  [context |-> vote.context,
   height |-> vote.height,
   view |-> AdequateLeaderFrozenViewCoordinate(vote.view, leaderView),
   phase |-> vote.phase,
   subject |-> vote.subject,
   signer |-> vote.signer]

AdequateLeaderFrozenTimeoutVotePayload(vote, leaderView) ==
  [context |-> vote.context,
   height |-> vote.height,
   view |-> AdequateLeaderFrozenViewCoordinate(vote.view, leaderView),
   signer |-> vote.signer,
   highestPrepareQc |->
     AdequateLeaderFrozenPrepareQcPayload(
       vote.highestPrepareQc, leaderView),
   highRank |->
     IF vote.highRank \in 0..leaderView
     THEN vote.highRank
     ELSE IF vote.highRank = NoRank THEN NoRank ELSE leaderView + 1,
   highSubject |-> vote.highSubject]

AdequateLeaderFrozenTcPayload(tc, leaderView) ==
  IF tc = NoTimeoutCertificate
  THEN NoTimeoutCertificate
  ELSE [context |-> tc.context,
        height |-> tc.height,
        view |->
          AdequateLeaderFrozenViewCoordinate(tc.view, leaderView),
        highestPrepareQc |->
          AdequateLeaderFrozenPrepareQcPayload(
            tc.highestPrepareQc, leaderView)]

AdequateLeaderFrozenProposalPayload(proposal, leaderView) ==
  [context |-> proposal.context,
   height |-> proposal.height,
   view |->
     AdequateLeaderFrozenViewCoordinate(proposal.view, leaderView),
   subject |-> proposal.subject,
   proposer |-> proposal.proposer,
   timeoutCertificate |->
     AdequateLeaderFrozenTcPayload(
       proposal.timeoutCertificate, leaderView),
   highestPrepareQc |->
     AdequateLeaderFrozenPrepareQcPayload(
       proposal.highestPrepareQc, leaderView),
   justifyRank |->
     IF proposal.justifyRank \in 0..leaderView
     THEN proposal.justifyRank
     ELSE IF proposal.justifyRank = NoRank
          THEN NoRank
          ELSE leaderView + 1,
   justifySubject |-> proposal.justifySubject]

AdequateLeaderFrozenBodyPayload(body, leaderView) ==
  [node |-> body.node,
   context |-> body.context,
   view |-> AdequateLeaderFrozenViewCoordinate(body.view, leaderView),
   subject |-> body.subject]

AdequateLeaderFrozenBodyEnvelopePayload(envelope, leaderView) ==
  [recipient |-> envelope.recipient,
   height |-> envelope.height,
   view |->
     AdequateLeaderFrozenViewCoordinate(envelope.view, leaderView),
   subject |-> envelope.subject,
   chunk |-> envelope.chunk,
   nonce |-> envelope.nonce]

AdequateLeaderFrozenCertifiedRequestHashPayload(requestHash, leaderView) ==
  LET signed == requestHash.exactSignedRequest
      preimage == signed.preimage
      signature == signed.signature
  IN [round |->
        [height |-> preimage.round.height,
         view |->
           AdequateLeaderFrozenViewCoordinate(
             preimage.round.view, leaderView)],
      subject |-> preimage.subject,
      certificate |->
        AdequateLeaderFrozenQcPayload(
          preimage.certificate, leaderView),
      requester |-> preimage.requester,
      signer |-> signature.signer,
      signatureNonce |-> signature.nonce]

AdequateLeaderFrozenCertifiedRequestItemPayload(item, leaderView) ==
  [recipient |-> item.envelope.recipient,
   height |-> item.envelope.height,
   view |->
     AdequateLeaderFrozenViewCoordinate(
       item.envelope.view, leaderView),
   subject |-> item.envelope.subject,
   requester |-> item.envelope.requester,
   certificate |->
     AdequateLeaderFrozenQcPayload(
       item.envelope.certificate, leaderView),
   signatureNonce |-> item.envelope.signatureNonce]

AdequateLeaderFrozenCommitRequestItemPayload(item, leaderView) ==
  [kind |-> item.kind,
   source |-> item.source,
   recipient |-> item.envelope.recipient,
   height |-> item.envelope.height,
   view |->
     AdequateLeaderFrozenViewCoordinate(
       item.envelope.view, leaderView),
   subject |-> item.envelope.subject,
   chunk |-> item.envelope.chunk,
   nonce |-> item.envelope.nonce]

AdequateLeaderFrozenCandidateItemPayload(item, leaderView) ==
  IF item = NoAsyncItem
  THEN [kind |-> "NoItem",
        source |-> 0,
        payload |-> NoAsyncItem]
  ELSE [kind |-> item.kind,
        \* Aggregate certificates and recovery responses may be relayed
        \* through a different physical source.  Their authenticated semantic
        \* certificate/archive fields remain exact below.
        source |->
          IF item.kind
               \in {"PrepareQC", "CommitQC", "TimeoutCertificate",
                    "CertifiedResponse", "CommitCertificateResponse"}
          THEN AsyncUntrustedSource
          ELSE item.source,
        payload |->
          CASE item.kind = "Proposal" ->
                 [recipient |-> item.envelope.recipient,
                  proposal |->
                    AdequateLeaderFrozenProposalPayload(
                      item.envelope.proposal, leaderView)]
            [] item.kind \in {"PrepareVote", "CommitVote"} ->
                 [recipient |-> item.envelope.recipient,
                  vote |->
                    AdequateLeaderFrozenVotePayload(
                      item.envelope.vote, leaderView)]
            [] item.kind \in {"PrepareQC", "CommitQC"} ->
                 [recipient |-> item.envelope.recipient,
                  qc |->
                    AdequateLeaderFrozenQcPayload(
                      item.envelope.qc, leaderView)]
            [] item.kind = "TimeoutVote" ->
                 [recipient |-> item.envelope.recipient,
                  vote |->
                    AdequateLeaderFrozenTimeoutVotePayload(
                      item.envelope.vote, leaderView)]
            [] item.kind = "TimeoutCertificate" ->
                 [recipient |-> item.envelope.recipient,
                  tc |->
                    AdequateLeaderFrozenTcPayload(
                      item.envelope.tc, leaderView)]
            [] item.kind = "CertifiedRequest" ->
                 AdequateLeaderFrozenCertifiedRequestItemPayload(
                   item, leaderView)
            [] item.kind = "CommitCertificateRequest" ->
                 AdequateLeaderFrozenCommitRequestItemPayload(
                   item, leaderView)
            [] item.kind = "CertifiedResponse" ->
                 [recipient |-> item.envelope.recipient,
                  height |-> item.envelope.height,
                  view |->
                    AdequateLeaderFrozenViewCoordinate(
                      item.envelope.view, leaderView),
                  subject |-> item.envelope.subject,
                  requestHash |->
                    AdequateLeaderFrozenCertifiedRequestHashPayload(
                      item.envelope.requestHash, leaderView),
                  archiveServer |-> item.envelope.archiveServer,
                  citedResponder |-> item.envelope.citedResponder,
                  signatureOwner |-> item.envelope.signatureOwner]
            [] item.kind = "CommitCertificateResponse" ->
                 [recipient |-> item.envelope.recipient,
                  request |->
                    AdequateLeaderFrozenCommitRequestItemPayload(
                      item.envelope.request, leaderView),
                  qc |->
                    AdequateLeaderFrozenQcPayload(
                      item.envelope.qc, leaderView)]
            [] OTHER ->
                 AdequateLeaderFrozenBodyEnvelopePayload(
                   item.envelope, leaderView)]

AdequateLeaderFrozenCandidateEvidencePayload(evidence, leaderView) ==
  IF evidence = NoAsyncItem
  THEN [kind |-> "NoEvidence", payload |-> NoAsyncItem]
  ELSE IF AsyncItemTyped(evidence)
       THEN [kind |-> "NetworkItem",
             payload |->
               AdequateLeaderFrozenCandidateItemPayload(
                 evidence, leaderView)]
       ELSE IF evidence \in ProposalRecordSet
            THEN [kind |-> "Proposal",
                  payload |->
                    AdequateLeaderFrozenProposalPayload(
                      evidence, leaderView)]
            ELSE IF evidence \in VoteRecordSet
                 THEN [kind |-> "Vote",
                       payload |->
                         AdequateLeaderFrozenVotePayload(
                           evidence, leaderView)]
                 ELSE IF evidence \in TimeoutVoteRecordSet
                      THEN [kind |-> "TimeoutVote",
                            payload |->
                              AdequateLeaderFrozenTimeoutVotePayload(
                                evidence, leaderView)]
                      ELSE IF evidence \in QcRecordSet
                           THEN [kind |-> "QC",
                                 payload |->
                                   AdequateLeaderFrozenQcPayload(
                                     evidence, leaderView)]
                           ELSE IF evidence \in TcRecordSet
                                THEN [kind |-> "TC",
                                      payload |->
                                        AdequateLeaderFrozenTcPayload(
                                          evidence, leaderView)]
                                ELSE [kind |-> "Body",
                                      payload |->
                                        AdequateLeaderFrozenBodyPayload(
                                          evidence, leaderView)]

AdequateLeaderFrozenCandidatePayload(candidate, leaderView) ==
  [class |-> candidate.class,
   workKind |-> candidate.kind,
   causalOrigin |-> candidate.causalOrigin,
   item |->
     AdequateLeaderFrozenCandidateItemPayload(
       candidate.item, leaderView),
   evidence |->
     AdequateLeaderFrozenCandidateEvidencePayload(
       candidate.evidence, leaderView),
   body |-> candidate.bodyIdentity,
   manifest |-> candidate.manifestIdentity,
   commitment |-> candidate.commitmentIdentity]

AdequateLeaderRouteNeutralCandidateItem(item) ==
  AsyncRouteNeutralCandidateItem(item)

AdequateLeaderRouteNeutralCandidateEvidence(evidence) ==
  AsyncRouteNeutralCandidateEvidence(evidence)

AdequateLeaderImmutableCandidatePayload(candidate) ==
  [class |-> candidate.class,
   workKind |-> candidate.kind,
   causalOrigin |-> candidate.causalOrigin,
   item |-> AdequateLeaderRouteNeutralCandidateItem(candidate.item),
   evidence |->
     AdequateLeaderRouteNeutralCandidateEvidence(candidate.evidence),
   body |-> candidate.bodyIdentity,
   manifest |-> candidate.manifestIdentity,
   commitment |-> candidate.commitmentIdentity]

\* The owner budget ranges over a static protocol/configuration carrier, not
\* over a state-derived candidate or wire set.  Each record universe below
\* caps every nested view at the frozen leader view before constructing an
\* exact wire item.  The broad CertifiedResponse constructor deliberately
\* includes both authenticated and aggregate-untrusted signature owners, so it
\* also contains the model's state-shaped transport-completion witness without
\* mentioning context, nodeView, a queue, or AsyncNetworkItems.
AdequateLeaderFrozenQcRecordCarrier(leaderView) ==
  {qc \in QcRecordSet: qc.view \in 0..leaderView}

AdequateLeaderFrozenVoteRecordCarrier(leaderView) ==
  {vote \in VoteRecordSet: vote.view \in 0..leaderView}

AdequateLeaderFrozenTimeoutVoteRecordCarrier(leaderView) ==
  {vote \in TimeoutVoteRecordSet:
     AdequateLeaderTimeoutVoteWithinFrozenView(vote, leaderView)}

AdequateLeaderFrozenTcRecordCarrier(leaderView) ==
  {tc \in TcRecordSet:
     AdequateLeaderTcWithinFrozenView(tc, leaderView)}

AdequateLeaderFrozenProposalRecordCarrier(leaderView) ==
  {proposal \in ProposalRecordSet:
     AdequateLeaderProposalWithinFrozenView(proposal, leaderView)}

AdequateLeaderFrozenBodyRecordCarrier(leaderView) ==
  {body \in BodyRecordSet: body.view \in 0..leaderView}

AdequateLeaderFrozenBodyEnvelopeCarrier(leaderView) ==
  [recipient: ValidatorIds,
   height: Heights,
   view: 0..leaderView,
   subject: Subjects,
   chunk: 0..AsyncChunkCount,
   nonce: 0..(AsyncIngressCapacity - 1)]

AdequateLeaderFrozenCertifiedRequestItemCarrier(leaderView) ==
  {AsyncNetworkItem(
     "CertifiedRequest", requester,
     AsyncCertifiedRequestEnvelope(route, requester, qc, signatureNonce)):
     requester \in ValidatorIds,
     route \in AsyncArchiveServerIds,
     qc \in AdequateLeaderFrozenQcRecordCarrier(leaderView),
     signatureNonce \in 0..(AsyncIngressCapacity - 1)}

AdequateLeaderFrozenCertifiedRequestHashCarrier(leaderView) ==
  {AsyncCertifiedRequestHash(request):
     request \in
       AdequateLeaderFrozenCertifiedRequestItemCarrier(leaderView)}

AdequateLeaderFrozenCommitRequestItemCarrier(leaderView) ==
  {AsyncNetworkItem("CommitCertificateRequest", source, envelope):
     source \in ValidatorIds,
     envelope \in
       {boundedEnvelope \in AsyncCommitCertificateRequestEnvelopeSet:
          boundedEnvelope.view \in 0..leaderView}}

AdequateLeaderFrozenNetworkItemCarrier(leaderView) ==
  {AsyncNetworkItem("Proposal", envelope.proposal.proposer, envelope):
     envelope \in
       [recipient: ValidatorIds,
        proposal:
          AdequateLeaderFrozenProposalRecordCarrier(leaderView)]}
  \cup
  {AsyncNetworkItem(
     IF envelope.vote.phase = "Prepare"
     THEN "PrepareVote"
     ELSE "CommitVote",
     envelope.vote.signer, envelope):
     envelope \in
       [recipient: ValidatorIds,
        vote: AdequateLeaderFrozenVoteRecordCarrier(leaderView)]}
  \cup
  {AsyncNetworkItem(
     IF envelope.qc.phase = "Prepare"
     THEN "PrepareQC"
     ELSE "CommitQC",
     source, envelope):
     source \in ValidatorIds,
     envelope \in
       [recipient: ValidatorIds,
        qc: AdequateLeaderFrozenQcRecordCarrier(leaderView)]}
  \cup
  {AsyncNetworkItem("TimeoutVote", envelope.vote.signer, envelope):
     envelope \in
       [recipient: ValidatorIds,
        vote:
          AdequateLeaderFrozenTimeoutVoteRecordCarrier(leaderView)]}
  \cup
  {AsyncNetworkItem("TimeoutCertificate", source, envelope):
     source \in ValidatorIds,
     envelope \in
       [recipient: ValidatorIds,
        tc: AdequateLeaderFrozenTcRecordCarrier(leaderView)]}
  \cup AdequateLeaderFrozenCertifiedRequestItemCarrier(leaderView)
  \cup AdequateLeaderFrozenCommitRequestItemCarrier(leaderView)
  \cup
  {AsyncNetworkItem(kind, source, envelope):
     kind \in {"NormalJunk", "ProgressJunk"},
     source \in ValidatorIds,
     envelope \in
       AdequateLeaderFrozenBodyEnvelopeCarrier(leaderView)}
  \cup
  {AsyncNetworkItem("Chunk", source, envelope):
     source \in AsyncIngressSources,
     envelope \in
       AdequateLeaderFrozenBodyEnvelopeCarrier(leaderView)}
  \cup
  {AsyncNetworkItem(
     "CertifiedResponse", source,
     [recipient |-> recipient,
      height |-> blockHeight,
      view |-> roundView,
      subject |-> responseSubject,
      requestHash |-> requestHash,
      archiveServer |-> archiveServer,
      citedResponder |-> citedResponder,
      signatureOwner |-> signatureOwner]):
     source \in AsyncIngressSources,
     recipient \in ValidatorIds,
     blockHeight \in Heights,
     roundView \in 0..leaderView,
     responseSubject \in Subjects,
     requestHash \in
       AdequateLeaderFrozenCertifiedRequestHashCarrier(leaderView),
     archiveServer \in AsyncArchiveServerIds,
     citedResponder \in ValidatorIds,
     signatureOwner \in AsyncCertifiedResponseSignatureOwners}
  \cup
  {AsyncNetworkItem(
     "CommitCertificateResponse", source,
     AsyncCommitCertificateResponseEnvelope(request, qc)):
     source \in AsyncIngressSources,
     request \in
       AdequateLeaderFrozenCommitRequestItemCarrier(leaderView),
     qc \in AdequateLeaderFrozenQcRecordCarrier(leaderView)}
  \cup
  {AsyncNetworkItem("Noise", source, envelope):
     source \in AsyncIngressSources,
     envelope \in
       AdequateLeaderFrozenBodyEnvelopeCarrier(leaderView)}

AdequateLeaderFrozenEvidenceCarrier(leaderView) ==
  AdequateLeaderFrozenNetworkItemCarrier(leaderView)
    \cup {NoAsyncItem}
    \cup AdequateLeaderFrozenProposalRecordCarrier(leaderView)
    \cup AdequateLeaderFrozenVoteRecordCarrier(leaderView)
    \cup AdequateLeaderFrozenTimeoutVoteRecordCarrier(leaderView)
    \cup AdequateLeaderFrozenQcRecordCarrier(leaderView)
    \cup AdequateLeaderFrozenTcRecordCarrier(leaderView)
    \cup AdequateLeaderFrozenBodyRecordCarrier(leaderView)

AdequateLeaderFrozenLifecycleNodes(target, leader) ==
  {target, leader}

AdequateLeaderFrozenLifecycleDeliveryItems(
    target, leaderContext, leader, leaderView) ==
  {item \in AdequateLeaderFrozenNetworkItemCarrier(leaderView):
     /\ item.envelope.recipient
          \in AdequateLeaderFrozenLifecycleNodes(target, leader)
     /\ DeliveryHeight(item) = leaderContext.height
     /\ DeliveryView(item) \in 0..leaderView}

AdequateLeaderFrozenDeliveryCausalOriginCarrier(
    target, leaderContext, leader, leaderView) ==
  {AsyncDeliveryCandidateCausalOriginAt(item, leaderContext):
     item \in AdequateLeaderFrozenLifecycleDeliveryItems(
                target, leaderContext, leader, leaderView)}

AdequateLeaderFrozenCertifiedResponseCausalOriginCarrier(
    target, leaderContext, leader, leaderView) ==
  {AsyncCertifiedResponseCandidateCausalOriginAt(item, leaderContext):
     item \in
       {response \in
          AdequateLeaderFrozenLifecycleDeliveryItems(
            target, leaderContext, leader, leaderView):
          response.kind = "CertifiedResponse"}}

AdequateLeaderFrozenCommitResponseCausalOriginCarrier(
    target, leaderContext, leader, leaderView) ==
  {AsyncCommitCertificateResponseCandidateCausalOriginAt(
     item, leaderContext):
     item \in
       {response \in
          AdequateLeaderFrozenLifecycleDeliveryItems(
            target, leaderContext, leader, leaderView):
          response.kind = "CommitCertificateResponse"}}

AdequateLeaderFrozenRestartQcCarrier(leaderContext, leaderView) ==
  {qc \in AdequateLeaderFrozenQcRecordCarrier(leaderView):
     qc.context = leaderContext}

AdequateLeaderFrozenRestartProposalCarrier(
    leaderContext, leaderView) ==
  {proposal \in AdequateLeaderFrozenProposalRecordCarrier(leaderView):
     proposal.context = leaderContext}

AdequateLeaderFrozenRestartVoteCarrier(leaderContext, leaderView) ==
  {vote \in AdequateLeaderFrozenVoteRecordCarrier(leaderView):
     vote.context = leaderContext}

AdequateLeaderFrozenRestartTimeoutVoteCarrier(
    leaderContext, leaderView) ==
  {vote \in AdequateLeaderFrozenTimeoutVoteRecordCarrier(leaderView):
     vote.context = leaderContext}

AdequateLeaderFrozenRestartCausalOriginCarrier(
    target, leaderContext, leader, leaderView) ==
  {AsyncRestartCandidateCausalOriginAt(
     "Completion", "FetchBody", node, leaderContext,
     qc.view, qc.subject, qc):
     node \in AdequateLeaderFrozenLifecycleNodes(target, leader),
     qc \in AdequateLeaderFrozenRestartQcCarrier(
              leaderContext, leaderView)}
  \cup
  {AsyncRestartCandidateCausalOriginAt(
     "Completion", "SignProposal", node, leaderContext,
     proposal.view, proposal.subject, proposal):
     node \in AdequateLeaderFrozenLifecycleNodes(target, leader),
     proposal \in AdequateLeaderFrozenRestartProposalCarrier(
                    leaderContext, leaderView)}
  \cup
  {AsyncRestartCandidateCausalOriginAt(
     "Completion", "SignVote", node, leaderContext,
     vote.view, vote.subject, vote):
     node \in AdequateLeaderFrozenLifecycleNodes(target, leader),
     vote \in AdequateLeaderFrozenRestartVoteCarrier(
                leaderContext, leaderView)}
  \cup
  {AsyncRestartCandidateCausalOriginAt(
     "Completion", "SignTimeout", node, leaderContext,
     vote.view, vote.highSubject, vote):
     node \in AdequateLeaderFrozenLifecycleNodes(target, leader),
     vote \in AdequateLeaderFrozenRestartTimeoutVoteCarrier(
                leaderContext, leaderView)}

AdequateLeaderFrozenHistoricalRetransmitCausalOriginCarrier(
    target, leaderContext, leader, leaderView) ==
  {AsyncHistoricalLockedRetransmitCandidateCausalOriginAt(
     node, leaderContext, qc):
     node \in AdequateLeaderFrozenLifecycleNodes(target, leader),
     qc \in AdequateLeaderFrozenRestartQcCarrier(
              leaderContext, leaderView)}

AdequateLeaderFrozenAssemblyCausalOriginCarrier(
    target, leaderContext, leader, leaderView) ==
  {AsyncNoItemCandidateCausalOriginAt(
     "Normal", "AssembleBody", node, leaderContext,
     roundView, ownerSubject):
     node \in AdequateLeaderFrozenLifecycleNodes(target, leader),
     roundView \in 0..leaderView,
     ownerSubject \in Subjects}

AdequateLeaderFrozenTimeoutCausalOriginCarrier(
    target, leaderContext, leader, leaderView) ==
  {AsyncNoItemCandidateCausalOriginAt(
     "Completion", "BeginTimeout", node, leaderContext,
     roundView, ownerSubject):
     node \in AdequateLeaderFrozenLifecycleNodes(target, leader),
     roundView \in 0..leaderView,
     ownerSubject \in SubjectOrNone}

\* This predicate is the proof-bearing constructor boundary.  It names the
\* seven concrete root families separately rather than accepting membership
\* in an arbitrary product of causal-origin fields.  The aggregate carrier
\* below is only their finite identity union.
AdequateLeaderFrozenCandidateRootConstructed(
    candidate, target, leaderContext, leader, leaderView) ==
  \/ candidate.causalOrigin
       \in AdequateLeaderFrozenDeliveryCausalOriginCarrier(
            target, leaderContext, leader, leaderView)
  \/ candidate.causalOrigin
       \in AdequateLeaderFrozenCertifiedResponseCausalOriginCarrier(
            target, leaderContext, leader, leaderView)
  \/ candidate.causalOrigin
       \in AdequateLeaderFrozenCommitResponseCausalOriginCarrier(
            target, leaderContext, leader, leaderView)
  \/ candidate.causalOrigin
       \in AdequateLeaderFrozenRestartCausalOriginCarrier(
            target, leaderContext, leader, leaderView)
  \/ candidate.causalOrigin
       \in AdequateLeaderFrozenHistoricalRetransmitCausalOriginCarrier(
            target, leaderContext, leader, leaderView)
  \/ candidate.causalOrigin
       \in AdequateLeaderFrozenAssemblyCausalOriginCarrier(
            target, leaderContext, leader, leaderView)
  \/ candidate.causalOrigin
       \in AdequateLeaderFrozenTimeoutCausalOriginCarrier(
            target, leaderContext, leader, leaderView)

\* A fixed adequate-leader episode can mint a causal root only through one
\* of the concrete scheduler constructors above.  Causal successors retain
\* that root verbatim.  This replaces the former arbitrary Cartesian product
\* of class, phase, item, and evidence fields: malformed combinations which
\* no transition can construct no longer inflate (or silently justify) the
\* owner budget.  BeginTimeout stays explicit and disjoint so the lifecycle
\* capacity proof can charge it to the dedicated clock slot.
AdequateLeaderFrozenCandidateCausalOriginCarrier(
    target, leaderContext, leader, leaderView, subject) ==
  IF /\ target \in ValidatorIds
     /\ leaderContext \in ContextRecords
     /\ leader \in ValidatorIds
     /\ leaderView \in Nat
     /\ subject \in Subjects
  THEN
    AdequateLeaderFrozenDeliveryCausalOriginCarrier(
      target, leaderContext, leader, leaderView)
      \cup
    AdequateLeaderFrozenCertifiedResponseCausalOriginCarrier(
      target, leaderContext, leader, leaderView)
      \cup
    AdequateLeaderFrozenCommitResponseCausalOriginCarrier(
      target, leaderContext, leader, leaderView)
      \cup
    AdequateLeaderFrozenRestartCausalOriginCarrier(
      target, leaderContext, leader, leaderView)
      \cup
    AdequateLeaderFrozenHistoricalRetransmitCausalOriginCarrier(
      target, leaderContext, leader, leaderView)
      \cup
    AdequateLeaderFrozenAssemblyCausalOriginCarrier(
      target, leaderContext, leader, leaderView)
      \cup
    AdequateLeaderFrozenTimeoutCausalOriginCarrier(
      target, leaderContext, leader, leaderView)
  ELSE {}

THEOREM AdequateLeaderFrozenCandidateRootConstructionCoversOrigin ==
  \A candidate, target, leaderContext, leader, leaderView, subject:
    /\ target \in ValidatorIds
    /\ leaderContext \in ContextRecords
    /\ leader \in ValidatorIds
    /\ leaderView \in Nat
    /\ subject \in Subjects
    /\ AdequateLeaderFrozenCandidateRootConstructed(
         candidate, target, leaderContext, leader, leaderView)
    => candidate.causalOrigin
         \in AdequateLeaderFrozenCandidateCausalOriginCarrier(
              target, leaderContext, leader, leaderView, subject)
BY Isa
   DEF AdequateLeaderFrozenCandidateRootConstructed,
       AdequateLeaderFrozenCandidateCausalOriginCarrier

AdequateLeaderFrozenCandidateItemPayloadCarrier(leaderView) ==
  {AdequateLeaderFrozenCandidateItemPayload(item, leaderView):
     item \in
       AdequateLeaderFrozenNetworkItemCarrier(leaderView)
         \cup {NoAsyncItem}}

AdequateLeaderFrozenCandidateEvidencePayloadCarrier(leaderView) ==
  {AdequateLeaderFrozenCandidateEvidencePayload(evidence, leaderView):
     evidence \in AdequateLeaderFrozenEvidenceCarrier(leaderView)}

AdequateLeaderFrozenCandidatePayloadCarrier(
    target, leaderContext, leader, leaderView, subject) ==
  IF /\ target \in ValidatorIds
     /\ leaderContext \in ContextRecords
     /\ leader \in ValidatorIds
     /\ leaderView \in Nat
     /\ subject \in Subjects
  THEN [class: AsyncCommandClasses,
        workKind: AsyncWorkKinds,
        causalOrigin:
          AdequateLeaderFrozenCandidateCausalOriginCarrier(
            target, leaderContext, leader, leaderView, subject),
        item: AdequateLeaderFrozenCandidateItemPayloadCarrier(leaderView),
        evidence:
          AdequateLeaderFrozenCandidateEvidencePayloadCarrier(leaderView),
        body: SubjectOrNone,
        manifest: SubjectOrNone,
        commitment: SubjectOrNone]
  ELSE {}

THEOREM AdequateLeaderFrozenTargetCandidatePayloadIsInStaticCarrier ==
  \A candidate, rank, target, leaderContext,
     leader, leaderView, subject:
    /\ candidate \in AsyncCandidateSet
    /\ target \in ValidatorIds
    /\ leaderContext \in ContextRecords
    /\ leader \in ValidatorIds
    /\ leaderView \in Nat
    /\ subject \in Subjects
    /\ AdequateLeaderFrozenTargetCandidateIdentity(
         candidate, rank, target, leaderContext,
         leader, leaderView, subject)
    => AdequateLeaderFrozenCandidatePayload(candidate, leaderView)
         \in AdequateLeaderFrozenCandidatePayloadCarrier(
              target, leaderContext, leader, leaderView, subject)
BY AdequateLeaderFrozenCandidateRootConstructionCoversOrigin,
   AsyncNetworkItemCarrierMemberIsTyped, IsaT(600)
   DEF AdequateLeaderFrozenCandidatePayloadCarrier,
       AdequateLeaderFrozenCandidateItemPayloadCarrier,
       AdequateLeaderFrozenCandidateEvidencePayloadCarrier,
       AdequateLeaderFrozenCandidateCausalOriginCarrier,
       AdequateLeaderFrozenCandidateRootConstructed,
       AdequateLeaderFrozenLifecycleNodes,
       AdequateLeaderFrozenLifecycleDeliveryItems,
       AdequateLeaderFrozenDeliveryCausalOriginCarrier,
       AdequateLeaderFrozenCertifiedResponseCausalOriginCarrier,
       AdequateLeaderFrozenCommitResponseCausalOriginCarrier,
       AdequateLeaderFrozenRestartQcCarrier,
       AdequateLeaderFrozenRestartProposalCarrier,
       AdequateLeaderFrozenRestartVoteCarrier,
       AdequateLeaderFrozenRestartTimeoutVoteCarrier,
       AdequateLeaderFrozenRestartCausalOriginCarrier,
       AdequateLeaderFrozenHistoricalRetransmitCausalOriginCarrier,
       AdequateLeaderFrozenAssemblyCausalOriginCarrier,
       AdequateLeaderFrozenTimeoutCausalOriginCarrier,
       AdequateLeaderFrozenEvidenceCarrier,
       AdequateLeaderFrozenNetworkItemCarrier,
       AdequateLeaderFrozenQcRecordCarrier,
       AdequateLeaderFrozenVoteRecordCarrier,
       AdequateLeaderFrozenTimeoutVoteRecordCarrier,
       AdequateLeaderFrozenTcRecordCarrier,
       AdequateLeaderFrozenProposalRecordCarrier,
       AdequateLeaderFrozenBodyRecordCarrier,
       AdequateLeaderFrozenBodyEnvelopeCarrier,
       AdequateLeaderFrozenCertifiedRequestItemCarrier,
       AdequateLeaderFrozenCertifiedRequestHashCarrier,
       AdequateLeaderFrozenCommitRequestItemCarrier,
       AdequateLeaderFrozenCandidatePayload,
       AdequateLeaderFrozenTargetCandidateIdentity,
       AdequateLeaderCandidatePayloadWithinFrozenView,
       AdequateLeaderCandidateItemWithinFrozenView,
       AdequateLeaderCandidateEvidenceWithinFrozenView,
       AsyncCandidateCausalOrigin,
       AsyncCandidateSet, AsyncNetworkItems, AsyncEvidenceSet

AdequateLeaderFrozenCandidateOwnerIdentityFromPayload(
    payload, owner, rank, target, leaderContext,
    leader, leaderView, subject) ==
  [target |-> target,
   context |-> leaderContext,
   leader |-> leader,
   view |-> leaderView,
   subject |-> subject,
   phase |-> rank,
   authority |->
     AdequateLeaderCorridorAuthorityReceipt(
       target, leaderContext, leader, leaderView),
   owner |-> owner,
   kind |-> "Candidate",
   payload |-> payload]

AdequateLeaderFrozenCandidateOwnerIdentity(
    candidate, rank, target, leaderContext, leader, leaderView, subject) ==
  AdequateLeaderFrozenCandidateOwnerIdentityFromPayload(
    AdequateLeaderFrozenCandidatePayload(candidate, leaderView),
    candidate.node, rank, target, leaderContext,
    leader, leaderView, subject)

THEOREM AdequateLeaderFrozenCandidateOwnerIdentitySeparatesPayload ==
  \A left, right, rank, target, leaderContext,
     leader, leaderView, subject:
    AdequateLeaderFrozenCandidateOwnerIdentity(
      left, rank, target, leaderContext, leader, leaderView, subject)
      =
    AdequateLeaderFrozenCandidateOwnerIdentity(
      right, rank, target, leaderContext, leader, leaderView, subject)
      => AdequateLeaderFrozenCandidatePayload(left, leaderView)
           = AdequateLeaderFrozenCandidatePayload(right, leaderView)
BY Isa
   DEF AdequateLeaderFrozenCandidateOwnerIdentity,
       AdequateLeaderFrozenCandidateOwnerIdentityFromPayload

THEOREM AdequateLeaderFrozenCandidateOwnerIdentityIsInjective ==
  \A left, right, rank, target, leaderContext,
     leader, leaderView, subject:
    /\ left \in AsyncCandidateSet
    /\ right \in AsyncCandidateSet
    /\ AdequateLeaderFrozenTargetCandidateIdentity(
         left, rank, target, leaderContext,
         leader, leaderView, subject)
    /\ AdequateLeaderFrozenTargetCandidateIdentity(
         right, rank, target, leaderContext,
         leader, leaderView, subject)
    /\ AdequateLeaderFrozenCandidateOwnerIdentity(
         left, rank, target, leaderContext,
         leader, leaderView, subject)
         =
       AdequateLeaderFrozenCandidateOwnerIdentity(
         right, rank, target, leaderContext,
         leader, leaderView, subject)
    => AdequateLeaderImmutableCandidatePayload(left)
         = AdequateLeaderImmutableCandidatePayload(right)
BY AdequateLeaderFrozenCandidateOwnerIdentitySeparatesPayload,
   AsyncNetworkItemCarrierMemberIsTyped,
   IsaT(600)
   DEF AdequateLeaderFrozenTargetCandidateIdentity,
       AdequateLeaderCandidatePayloadWithinFrozenView,
       AdequateLeaderCandidateItemWithinFrozenView,
       AdequateLeaderCandidateEvidenceWithinFrozenView,
       AdequateLeaderPrepareQcWithinFrozenView,
       AdequateLeaderTimeoutVoteWithinFrozenView,
       AdequateLeaderTcWithinFrozenView,
       AdequateLeaderProposalWithinFrozenView,
       AdequateLeaderCertifiedRequestHashWithinFrozenView,
       AdequateLeaderFrozenCandidatePayload,
       AdequateLeaderFrozenCandidateItemPayload,
       AdequateLeaderFrozenCandidateEvidencePayload,
       AsyncCandidateCausalOrigin,
       AdequateLeaderFrozenViewCoordinate,
       AdequateLeaderFrozenQcPayload,
       AdequateLeaderFrozenPrepareQcPayload,
       AdequateLeaderFrozenVotePayload,
       AdequateLeaderFrozenTimeoutVotePayload,
       AdequateLeaderFrozenTcPayload,
       AdequateLeaderFrozenProposalPayload,
       AdequateLeaderFrozenBodyPayload,
       AdequateLeaderFrozenBodyEnvelopePayload,
       AdequateLeaderFrozenCertifiedRequestHashPayload,
       AdequateLeaderFrozenCertifiedRequestItemPayload,
       AdequateLeaderFrozenCommitRequestItemPayload,
       AdequateLeaderImmutableCandidatePayload,
       AdequateLeaderRouteNeutralCandidateItem,
       AdequateLeaderRouteNeutralCandidateEvidence,
       AsyncRouteNeutralCandidateItem,
       AsyncRouteNeutralCandidateEvidence,
       AsyncCandidateQcSemanticPayload,
       AsyncCandidatePrepareQcSemanticPayload,
       AsyncCandidateVoteSemanticPayload,
       AsyncCandidateTimeoutVoteSemanticPayload,
       AsyncCandidateTcSemanticPayload,
       AsyncCandidateProposalSemanticPayload,
       AsyncCandidateCertifiedRequestHashSemanticPayload,
       AsyncCandidateCertifiedRequestItemSemanticPayload,
       AsyncCandidateCommitRequestItemSemanticPayload,
       CertificateRefOf,
       AsyncCandidateSet, AsyncNetworkItems, AsyncEvidenceSet

THEOREM AdequateLeaderFrozenCandidateRetryIdentityIsStable ==
  \A left, right, rank, target, leaderContext,
     leader, leaderView, subject:
    /\ left.node = right.node
    /\ AdequateLeaderCandidatePayloadWithinFrozenView(
         left, leaderView)
    /\ AdequateLeaderCandidatePayloadWithinFrozenView(
         right, leaderView)
    /\ AdequateLeaderImmutableCandidatePayload(left)
         = AdequateLeaderImmutableCandidatePayload(right)
    => AdequateLeaderFrozenCandidateOwnerIdentity(
         left, rank, target, leaderContext,
         leader, leaderView, subject)
         =
       AdequateLeaderFrozenCandidateOwnerIdentity(
         right, rank, target, leaderContext,
         leader, leaderView, subject)
BY IsaT(600)
   DEF AdequateLeaderFrozenCandidateOwnerIdentity,
       AdequateLeaderFrozenCandidateOwnerIdentityFromPayload,
       AdequateLeaderCandidatePayloadWithinFrozenView,
       AdequateLeaderCandidateItemWithinFrozenView,
       AdequateLeaderCandidateEvidenceWithinFrozenView,
       AdequateLeaderPrepareQcWithinFrozenView,
       AdequateLeaderTimeoutVoteWithinFrozenView,
       AdequateLeaderTcWithinFrozenView,
       AdequateLeaderProposalWithinFrozenView,
       AdequateLeaderCertifiedRequestHashWithinFrozenView,
       AdequateLeaderFrozenCandidatePayload,
       AdequateLeaderFrozenCandidateItemPayload,
       AdequateLeaderFrozenCandidateEvidencePayload,
       AdequateLeaderFrozenViewCoordinate,
       AdequateLeaderFrozenQcPayload,
       AdequateLeaderFrozenPrepareQcPayload,
       AdequateLeaderFrozenVotePayload,
       AdequateLeaderFrozenTimeoutVotePayload,
       AdequateLeaderFrozenTcPayload,
       AdequateLeaderFrozenProposalPayload,
       AdequateLeaderFrozenBodyPayload,
       AdequateLeaderFrozenBodyEnvelopePayload,
       AdequateLeaderFrozenCertifiedRequestHashPayload,
       AdequateLeaderFrozenCertifiedRequestItemPayload,
       AdequateLeaderFrozenCommitRequestItemPayload,
       AdequateLeaderImmutableCandidatePayload,
       AdequateLeaderRouteNeutralCandidateItem,
       AdequateLeaderRouteNeutralCandidateEvidence,
       AsyncRouteNeutralCandidateItem,
       AsyncRouteNeutralCandidateEvidence,
       AsyncCandidateQcSemanticPayload,
       AsyncCandidatePrepareQcSemanticPayload,
       AsyncCandidateVoteSemanticPayload,
       AsyncCandidateTimeoutVoteSemanticPayload,
       AsyncCandidateTcSemanticPayload,
       AsyncCandidateProposalSemanticPayload,
       AsyncCandidateCertifiedRequestHashSemanticPayload,
       AsyncCandidateCertifiedRequestItemSemanticPayload,
       AsyncCandidateCommitRequestItemSemanticPayload,
       CertificateRefOf

AdequateLeaderTargetRankOwnerIdentitySet(
    target, leaderContext, leader, leaderView, subject, rank) ==
  {AdequateLeaderFrozenCandidateOwnerIdentity(
     candidate, rank, target, leaderContext,
     leader, leaderView, subject):
     candidate \in
       AdequateLeaderTargetRankOwnerSet(
         target, leaderContext, leader, leaderView, subject, rank)}

\* This is a distinct-logical-owner count.  On `AsyncLiveSpecAt` traces,
\* `AsyncProgressOwnershipInvariant` rules out two scheduler locations owning
\* one equal candidate value; the count must not be read as a physical-copy
\* theorem in an arbitrary typed state.
AdequateLeaderTargetRankOwnerCount(
    target, leaderContext, leader, leaderView, subject, rank) ==
  Cardinality(
    AdequateLeaderTargetRankOwnerIdentitySet(
      target, leaderContext, leader, leaderView, subject, rank))

AdequateLeaderTargetOccurrenceRankCarrier ==
  AdequateLeaderTargetSemanticRankCarrier \X Nat

AdequateLeaderTargetOccurrenceRankOrdering ==
  LexPairOrdering(
    AdequateLeaderTargetSemanticRankOrdering,
    OpToRel(<, Nat),
    AdequateLeaderTargetSemanticRankCarrier,
    Nat)

AdequateLeaderTargetOccurrenceRankFrontier(
    target, leaderContext, leader, leaderView, subject, occurrenceRank) ==
  /\ occurrenceRank \in AdequateLeaderTargetOccurrenceRankCarrier
  /\ occurrenceRank[2] > 0
  /\ IsFiniteSet(
       AdequateLeaderTargetRankOwnerSet(
         target, leaderContext, leader, leaderView,
         subject, occurrenceRank[1]))
  /\ AdequateLeaderTargetRankFrontier(
       target, leaderContext, leader, leaderView, subject, occurrenceRank[1])
  /\ occurrenceRank[2] =
       AdequateLeaderTargetRankOwnerCount(
         target, leaderContext, leader, leaderView,
         subject, occurrenceRank[1])

AdequateLeaderTargetOccurrenceFrontierRankSet(
    target, leaderContext, leader, leaderView, subject) ==
  {occurrenceRank \in AdequateLeaderTargetOccurrenceRankCarrier:
     AdequateLeaderTargetOccurrenceRankFrontier(
       target, leaderContext, leader, leaderView,
       subject, occurrenceRank)}

THEOREM AdequateLeaderTargetOccurrenceFrontierProjectsSemanticFrontier ==
  \A target, leaderContext, leader, leaderView, subject, occurrenceRank:
    AdequateLeaderTargetOccurrenceRankFrontier(
      target, leaderContext, leader, leaderView, subject, occurrenceRank)
      => AdequateLeaderTargetRankFrontier(
           target, leaderContext, leader, leaderView,
           subject, occurrenceRank[1])
BY DEF AdequateLeaderTargetOccurrenceRankFrontier

THEOREM AdequateLeaderDecisionPhaseFrontierIsTargetOwned ==
  \A candidate, rank, target, leaderContext, leader, leaderView, subject:
    /\ AdequateLeaderTargetCandidateIdentity(
         candidate, rank, target, leaderContext,
         leader, leaderView, subject)
    /\ candidate.kind \in {"BeginDecision", "PersistDecision"}
    => candidate.node = target
BY DEF AdequateLeaderTargetCandidateIdentity,
       AdequateLeaderTargetCandidateRole

THEOREM AdequateLeaderOtherNodePostDecisionRecoveryIsNotTargetFrontier ==
  \A candidate, rank, target, leaderContext, leader, leaderView, subject:
    /\ candidate.node # target
    /\ NodeHasDecision(candidate.node)
    => ~AdequateLeaderTargetCandidateIdentity(
          candidate, rank, target, leaderContext,
          leader, leaderView, subject)
BY Isa
   DEF AdequateLeaderTargetCandidateIdentity,
       AdequateLeaderTargetCandidateRole

THEOREM AdequateLeaderTargetPersistDecisionExecutionReachesIndexedGoal ==
  \A candidate, rank, target, leaderContext, leader, leaderView, subject:
    /\ AdequateLeaderTargetCandidateIdentity(
         candidate, rank, target, leaderContext,
         leader, leaderView, subject)
    /\ candidate.kind = "PersistDecision"
    /\ ExecutePersistDecision(candidate)
    => NodeHasDecision(target)'
BY AdequateLeaderDecisionPhaseFrontierIsTargetOwned,
   ExecutePersistDecisionCreatesExactDecisionMilestone, Isa

AdequateLeaderFrozenTargetWireIdentity(
    item, target, leaderContext, leader, leaderView, subject) ==
  /\ leaderContext \in ContextRecords
  /\ leaderView \in Views
  /\ subject \in Subjects
  /\ item.kind \in LeaderWireKinds
  /\ item.envelope.recipient \in {target, leader}
  /\ DeliveryView(item) = leaderView
  /\ DeliverySubject(item) = subject
  /\ LeaderWireCarriesContext(item, leaderContext)
  /\ IF item.kind = "CertifiedResponse"
     THEN /\ item.envelope.archiveServer \in AsyncArchiveServerIds
          /\ item.envelope.signatureOwner =
               item.envelope.archiveServer
     ELSE TRUE

AdequateLeaderTargetWireIdentity(
    item, target, leaderContext, leader, leaderView, subject) ==
  /\ AdequateLeaderFrozenTargetCorridor(
       target, leaderContext, leader, leaderView)
  /\ AdequateLeaderFrozenTargetWireIdentity(
       item, target, leaderContext, leader, leaderView, subject)
  /\ LeaderWireExactSemanticIdentity(
       item, leaderContext, item.envelope.recipient,
       leaderView, subject)
  /\ LeaderWireProductiveTransportIdentity(item)

\* A certified response may be relayed through another ingress source while
\* retaining the same signed archive/request binding, so only that kind
\* normalizes its physical route.  Authenticated control sources and chunk
\* sources remain part of the frozen owner identity.
AdequateLeaderFrozenWirePayloadIdentity(item) ==
  [source |->
     IF item.kind = "CertifiedResponse"
     THEN AsyncUntrustedSource
     ELSE item.source,
   detail |->
     CASE item.kind = "CertifiedResponse" ->
            item.envelope.archiveServer
       [] item.kind = "Chunk" -> item.envelope.chunk
       [] OTHER -> NoAsyncChunk]

AdequateLeaderFrozenWirePayloadCarrier ==
  [source: AsyncIngressSources,
   detail: AsyncArchiveServerIds \cup AsyncChunks \cup {NoAsyncChunk}]

AdequateLeaderFrozenWireOwnerIdentityFromCoordinates(
    wireKind, recipient, payload, target,
    leaderContext, leader, leaderView, subject) ==
  [target |-> target,
   context |-> leaderContext,
   leader |-> leader,
   view |-> leaderView,
   subject |-> subject,
   phase |-> wireKind,
   authority |->
     AdequateLeaderCorridorAuthorityReceipt(
       target, leaderContext, leader, leaderView),
   owner |-> recipient,
   kind |-> "Wire",
   payload |-> payload]

AdequateLeaderFrozenWireOwnerIdentity(
    item, target, leaderContext, leader, leaderView, subject) ==
  AdequateLeaderFrozenWireOwnerIdentityFromCoordinates(
    item.kind, item.envelope.recipient,
    AdequateLeaderFrozenWirePayloadIdentity(item),
    target, leaderContext, leader, leaderView, subject)

AdequateLeaderFrozenCandidateOwnerUniverse(
    target, leaderContext, leader, leaderView, subject) ==
  {AdequateLeaderFrozenCandidateOwnerIdentityFromPayload(
     payload, owner, rank, target, leaderContext,
     leader, leaderView, subject):
     payload \in
       AdequateLeaderFrozenCandidatePayloadCarrier(
         target, leaderContext, leader, leaderView, subject),
     owner \in {target, leader},
     rank \in AdequateLeaderTargetSemanticRankCarrier}

AdequateLeaderFrozenWireOwnerUniverse(
    target, leaderContext, leader, leaderView, subject) ==
  {AdequateLeaderFrozenWireOwnerIdentityFromCoordinates(
     wireKind, recipient, payload, target,
     leaderContext, leader, leaderView, subject):
     wireKind \in LeaderWireKinds,
     recipient \in {target, leader},
     payload \in AdequateLeaderFrozenWirePayloadCarrier}

THEOREM AdequateLeaderFrozenCandidateOwnerCarriesAuthorityReceipt ==
  \A owner, target, leaderContext, leader, leaderView, subject:
    owner \in
      AdequateLeaderFrozenCandidateOwnerUniverse(
        target, leaderContext, leader, leaderView, subject)
      => owner.authority =
           AdequateLeaderCorridorAuthorityReceipt(
             target, leaderContext, leader, leaderView)
BY Isa
   DEF AdequateLeaderFrozenCandidateOwnerUniverse,
       AdequateLeaderFrozenCandidateOwnerIdentityFromPayload

THEOREM AdequateLeaderFrozenWireOwnerCarriesAuthorityReceipt ==
  \A owner, target, leaderContext, leader, leaderView, subject:
    owner \in
      AdequateLeaderFrozenWireOwnerUniverse(
        target, leaderContext, leader, leaderView, subject)
      => owner.authority =
           AdequateLeaderCorridorAuthorityReceipt(
             target, leaderContext, leader, leaderView)
BY Isa
   DEF AdequateLeaderFrozenWireOwnerUniverse,
       AdequateLeaderFrozenWireOwnerIdentityFromCoordinates

AdequateLeaderFrozenOwnerUniverse(
    target, leaderContext, leader, leaderView, subject) ==
  AdequateLeaderFrozenCandidateOwnerUniverse(
    target, leaderContext, leader, leaderView, subject)
    \cup
  AdequateLeaderFrozenWireOwnerUniverse(
    target, leaderContext, leader, leaderView, subject)

\* Subject rotation is bounded by exact immutable owners, not by treating a
\* change of subject as protocol progress.  The frozen target/context/leader/
\* view coordinates remain fixed while the union ranges over the finite model
\* subject carrier; each member still retains its own subject and phase.
AdequateLeaderFrozenSubjectSwitchOwnerUniverse(
    target, leaderContext, leader, leaderView) ==
  UNION {
    AdequateLeaderFrozenOwnerUniverse(
      target, leaderContext, leader, leaderView, subject):
    subject \in Subjects}

AdequateLeaderTargetLiveCandidateOwnerIdentitySet(
    target, leaderContext, leader, leaderView, subject) ==
  UNION {
    AdequateLeaderTargetRankOwnerIdentitySet(
      target, leaderContext, leader, leaderView, subject, rank):
    rank \in AdequateLeaderTargetSemanticRankCarrier}

AdequateLeaderTargetLiveWireOwnerIdentitySet(
    target, leaderContext, leader, leaderView, subject) ==
  {AdequateLeaderFrozenWireOwnerIdentity(
     item, target, leaderContext, leader, leaderView, subject):
     item \in
       {wire \in AsyncNetworkItems:
          /\ AdequateLeaderTargetWireIdentity(
               wire, target, leaderContext,
               leader, leaderView, subject)
          /\ LeaderWireLogicalServiceActive(wire)
          /\ \/ ItemHasPacket(wire)
             \/ LeaderWireIngressOwned(wire)
             \/ LeaderWireCandidateOwned(wire)
             \/ LeaderWireLiveControlServiceOwner(wire)}}

AdequateLeaderTargetLiveOwnerIdentitySet(
    target, leaderContext, leader, leaderView, subject) ==
  AdequateLeaderTargetLiveCandidateOwnerIdentitySet(
    target, leaderContext, leader, leaderView, subject)
    \cup
  AdequateLeaderTargetLiveWireOwnerIdentitySet(
    target, leaderContext, leader, leaderView, subject)

THEOREM AdequateLeaderFrozenOwnerUniverseIsPrimeInvariant ==
  \A target, leaderContext, leader, leaderView, subject:
    AdequateLeaderFrozenOwnerUniverse(
      target, leaderContext, leader, leaderView, subject)'
      = AdequateLeaderFrozenOwnerUniverse(
          target, leaderContext, leader, leaderView, subject)
BY Isa
   DEF AdequateLeaderFrozenOwnerUniverse,
       AdequateLeaderFrozenCandidateOwnerUniverse,
       AdequateLeaderFrozenWireOwnerUniverse,
       AdequateLeaderFrozenCandidateOwnerIdentityFromPayload,
       AdequateLeaderFrozenCandidatePayloadCarrier,
       AdequateLeaderFrozenCandidateItemPayloadCarrier,
       AdequateLeaderFrozenCandidateEvidencePayloadCarrier,
       AdequateLeaderFrozenCandidateCausalOriginCarrier,
       AdequateLeaderFrozenLifecycleNodes,
       AdequateLeaderFrozenLifecycleDeliveryItems,
       AdequateLeaderFrozenDeliveryCausalOriginCarrier,
       AdequateLeaderFrozenCertifiedResponseCausalOriginCarrier,
       AdequateLeaderFrozenCommitResponseCausalOriginCarrier,
       AdequateLeaderFrozenRestartQcCarrier,
       AdequateLeaderFrozenRestartProposalCarrier,
       AdequateLeaderFrozenRestartVoteCarrier,
       AdequateLeaderFrozenRestartTimeoutVoteCarrier,
       AdequateLeaderFrozenRestartCausalOriginCarrier,
       AdequateLeaderFrozenHistoricalRetransmitCausalOriginCarrier,
       AdequateLeaderFrozenAssemblyCausalOriginCarrier,
       AdequateLeaderFrozenTimeoutCausalOriginCarrier,
       AdequateLeaderFrozenEvidenceCarrier,
       AdequateLeaderFrozenNetworkItemCarrier,
       AdequateLeaderFrozenQcRecordCarrier,
       AdequateLeaderFrozenVoteRecordCarrier,
       AdequateLeaderFrozenTimeoutVoteRecordCarrier,
       AdequateLeaderFrozenTcRecordCarrier,
       AdequateLeaderFrozenProposalRecordCarrier,
       AdequateLeaderFrozenBodyRecordCarrier,
       AdequateLeaderFrozenBodyEnvelopeCarrier,
       AdequateLeaderFrozenCertifiedRequestItemCarrier,
       AdequateLeaderFrozenCertifiedRequestHashCarrier,
       AdequateLeaderFrozenCommitRequestItemCarrier,
       AdequateLeaderFrozenCandidateItemPayload,
       AdequateLeaderFrozenCandidateEvidencePayload,
       AsyncCandidateCausalOrigin,
       AdequateLeaderFrozenWireOwnerIdentityFromCoordinates,
       AdequateLeaderFrozenWirePayloadCarrier

THEOREM AdequateLeaderFrozenSubjectSwitchOwnerUniverseIsPrimeInvariant ==
  \A target, leaderContext, leader, leaderView:
    AdequateLeaderFrozenSubjectSwitchOwnerUniverse(
      target, leaderContext, leader, leaderView)'
      = AdequateLeaderFrozenSubjectSwitchOwnerUniverse(
          target, leaderContext, leader, leaderView)
BY AdequateLeaderFrozenOwnerUniverseIsPrimeInvariant, Isa
   DEF AdequateLeaderFrozenSubjectSwitchOwnerUniverse

THEOREM AdequateLeaderFrozenCandidatePayloadCarrierIsFinite ==
  \A target, leaderContext, leader, leaderView, subject:
    /\ target \in ValidatorIds
    /\ leaderContext \in ContextRecords
    /\ leader \in ValidatorIds
    /\ leaderView \in Nat
    /\ subject \in Subjects
    => IsFiniteSet(
         AdequateLeaderFrozenCandidatePayloadCarrier(
           target, leaderContext, leader, leaderView, subject))
BY FS_Interval, FS_Image, FS_Union, FS_Subset, FS_Product, IsaT(600)
   DEF AdequateLeaderFrozenCandidatePayloadCarrier,
       AdequateLeaderFrozenCandidateItemPayloadCarrier,
       AdequateLeaderFrozenCandidateEvidencePayloadCarrier,
       AdequateLeaderFrozenCandidateCausalOriginCarrier,
       AdequateLeaderFrozenLifecycleNodes,
       AdequateLeaderFrozenLifecycleDeliveryItems,
       AdequateLeaderFrozenDeliveryCausalOriginCarrier,
       AdequateLeaderFrozenCertifiedResponseCausalOriginCarrier,
       AdequateLeaderFrozenCommitResponseCausalOriginCarrier,
       AdequateLeaderFrozenRestartQcCarrier,
       AdequateLeaderFrozenRestartProposalCarrier,
       AdequateLeaderFrozenRestartVoteCarrier,
       AdequateLeaderFrozenRestartTimeoutVoteCarrier,
       AdequateLeaderFrozenRestartCausalOriginCarrier,
       AdequateLeaderFrozenHistoricalRetransmitCausalOriginCarrier,
       AdequateLeaderFrozenAssemblyCausalOriginCarrier,
       AdequateLeaderFrozenTimeoutCausalOriginCarrier,
       AdequateLeaderFrozenEvidenceCarrier,
       AdequateLeaderFrozenNetworkItemCarrier,
       AdequateLeaderFrozenQcRecordCarrier,
       AdequateLeaderFrozenVoteRecordCarrier,
       AdequateLeaderFrozenTimeoutVoteRecordCarrier,
       AdequateLeaderFrozenTcRecordCarrier,
       AdequateLeaderFrozenProposalRecordCarrier,
       AdequateLeaderFrozenBodyRecordCarrier,
       AdequateLeaderFrozenBodyEnvelopeCarrier,
       AdequateLeaderFrozenCertifiedRequestItemCarrier,
       AdequateLeaderFrozenCertifiedRequestHashCarrier,
       AdequateLeaderFrozenCommitRequestItemCarrier,
       AdequateLeaderFrozenCandidateItemPayload,
       AdequateLeaderFrozenCandidateEvidencePayload,
       AdequateLeaderFrozenViewCoordinate,
       AdequateLeaderFrozenQcPayload,
       AdequateLeaderFrozenPrepareQcPayload,
       AdequateLeaderFrozenVotePayload,
       AdequateLeaderFrozenTimeoutVotePayload,
       AdequateLeaderFrozenTcPayload,
       AdequateLeaderFrozenProposalPayload,
       AdequateLeaderFrozenBodyPayload,
       AdequateLeaderFrozenBodyEnvelopePayload,
       AdequateLeaderFrozenCertifiedRequestHashPayload,
       AdequateLeaderFrozenCertifiedRequestItemPayload,
       AdequateLeaderFrozenCommitRequestItemPayload

THEOREM AdequateLeaderFrozenOwnerUniverseIsFinite ==
  \A target, leaderContext, leader, leaderView, subject:
    /\ target \in ValidatorIds
    /\ leaderContext \in ContextRecords
    /\ leader \in ValidatorIds
    /\ leaderView \in Nat
    /\ subject \in Subjects
    => /\ IsFiniteSet(
         AdequateLeaderFrozenOwnerUniverse(
           target, leaderContext, leader, leaderView, subject))
       /\ Cardinality(
            AdequateLeaderFrozenOwnerUniverse(
              target, leaderContext, leader, leaderView, subject))
            <= 2 * Cardinality(
                 AdequateLeaderTargetSemanticRankCarrier)
                   * Cardinality(
                       AdequateLeaderFrozenCandidatePayloadCarrier(
                         target, leaderContext, leader,
                         leaderView, subject))
                 + 2 * Cardinality(LeaderWireKinds)
                     * Cardinality(
                         AdequateLeaderFrozenWirePayloadCarrier)
BY AdequateLeaderFrozenCandidatePayloadCarrierIsFinite,
   FS_Union, FS_Product, FS_CardinalityType, IsaT(240)
   DEF AdequateLeaderFrozenOwnerUniverse,
       AdequateLeaderFrozenCandidateOwnerUniverse,
       AdequateLeaderFrozenWireOwnerUniverse,
       AdequateLeaderFrozenWirePayloadCarrier

(***************************************************************************
Proofless reordered-Chunk bridge.

Before a retained Proposal binds manifest coordinates, a Chunk owns ordinary
bounded ingress rather than a global leader-wire scheduler ordinal.  It is
still one member of the same immutable adequate-leader wire universe: its
authenticated physical source and finite chunk index are the frozen payload.
The source model charges at most one full ingress-capacity prefix plus the
route-neutral finite receipt set for a fixed recipient/view/subject episode.
Thus this arm may be serviced as a finite non-descent producer episode, but is
never itself called Decision or occurrence-rank progress.
***************************************************************************)
AdequateLeaderProoflessChunkIngressOwner(
    item, target, leaderContext, leader, leaderView, subject) ==
  /\ AdequateLeaderTargetWireIdentity(
       item, target, leaderContext, leader, leaderView, subject)
  /\ item.kind = "Chunk"
  /\ LeaderWireIngressOwned(item)
  /\ ~AsyncChunkExactLifecycleCoordinatesRetained(item)

THEOREM AdequateLeaderProoflessChunkIngressOwnerIsFrozen ==
  \A item, target, leaderContext, leader, leaderView, subject:
    AdequateLeaderProoflessChunkIngressOwner(
      item, target, leaderContext, leader, leaderView, subject)
      => AdequateLeaderFrozenWireOwnerIdentity(
           item, target, leaderContext, leader, leaderView, subject)
           \in AdequateLeaderFrozenOwnerUniverse(
                target, leaderContext, leader, leaderView, subject)
BY Isa
   DEF AdequateLeaderProoflessChunkIngressOwner,
       AdequateLeaderTargetWireIdentity,
       AdequateLeaderFrozenTargetWireIdentity,
       AdequateLeaderFrozenWireOwnerIdentity,
       AdequateLeaderFrozenWireOwnerUniverse,
       AdequateLeaderFrozenOwnerUniverse,
       AdequateLeaderFrozenWirePayloadIdentity,
       AdequateLeaderFrozenWirePayloadCarrier

THEOREM AdequateLeaderProoflessChunkEpisodeBudgetIsFiniteAndCoalesced ==
  \A target, leaderContext, leader, leaderView, subject:
    \A node \in {target, leader}:
      /\ AsyncStrongTypeInvariant
      /\ AdequateLeaderFrozenTargetCorridor(
           target, leaderContext, leader, leaderView)
      /\ subject \in Subjects
      => /\ IsFiniteSet(
               AsyncProoflessChunkEpisodeDebtSet(
                 node, leaderView, subject))
         /\ Cardinality(
              AsyncProoflessChunkEpisodeDebtSet(
                node, leaderView, subject))
              <= AsyncIngressCapacity + AsyncChunkCount
BY AsyncStrongTypeProjectsAsyncType,
   AsyncProoflessChunkEpisodeBudgetIsFiniteAndCoalesced, Isa
   DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncIngressTypeInvariant, AsyncTransportTypeInvariant,
       AsyncTransportContentTypeInvariant

THEOREM AdequateLeaderFrozenSubjectSwitchOwnerUniverseIsFinite ==
  \A target, leaderContext, leader, leaderView:
    /\ target \in ValidatorIds
    /\ leaderContext \in ContextRecords
    /\ leader \in ValidatorIds
    /\ leaderView \in Nat
    => IsFiniteSet(
         AdequateLeaderFrozenSubjectSwitchOwnerUniverse(
           target, leaderContext, leader, leaderView))
BY AdequateLeaderFrozenOwnerUniverseIsFinite,
   FS_Union, Isa
   DEF AdequateLeaderFrozenSubjectSwitchOwnerUniverse,
       Subjects

THEOREM AdequateLeaderLiveOwnersStayInsideFrozenUniverse ==
  \A target, leaderContext, leader, leaderView, subject:
    /\ AsyncStrongTypeInvariant
    /\ AdequateLeaderFrozenTargetCorridor(
         target, leaderContext, leader, leaderView)
    => AdequateLeaderTargetLiveOwnerIdentitySet(
         target, leaderContext, leader, leaderView, subject)
         \subseteq
           AdequateLeaderFrozenOwnerUniverse(
             target, leaderContext, leader, leaderView, subject)
BY AdequateLeaderFrozenTargetCandidatePayloadIsInStaticCarrier, IsaT(300)
   DEF AdequateLeaderTargetLiveOwnerIdentitySet,
       AdequateLeaderTargetLiveCandidateOwnerIdentitySet,
       AdequateLeaderTargetLiveWireOwnerIdentitySet,
       AdequateLeaderTargetRankOwnerIdentitySet,
       AdequateLeaderTargetRankOwnerSet,
       AdequateLeaderTargetCandidateIdentity,
       AdequateLeaderFrozenTargetCandidateIdentity,
       AdequateLeaderTargetCandidateRole,
       AdequateLeaderFrozenTargetCandidateRole,
       AdequateLeaderFrozenCandidateOwnerIdentity,
       AdequateLeaderFrozenCandidatePayload,
       AdequateLeaderFrozenCandidatePayloadCarrier,
       AdequateLeaderFrozenCandidateOwnerUniverse,
       AdequateLeaderFrozenWireOwnerIdentity,
       AdequateLeaderFrozenWireOwnerUniverse,
       AdequateLeaderFrozenWirePayloadIdentity,
       AdequateLeaderFrozenWirePayloadCarrier,
       AdequateLeaderFrozenOwnerUniverse

THEOREM AdequateLeaderFrozenWireRetryIdentityIsStable ==
  \A left, right, target, leaderContext, leader, leaderView, subject:
    /\ AdequateLeaderFrozenTargetWireIdentity(
         left, target, leaderContext, leader, leaderView, subject)
    /\ AdequateLeaderFrozenTargetWireIdentity(
         right, target, leaderContext, leader, leaderView, subject)
    /\ left.kind = right.kind
    /\ (left.kind = "CertifiedResponse"
          \/ left.source = right.source)
    /\ left.envelope = right.envelope
    => AdequateLeaderFrozenWireOwnerIdentity(
         left, target, leaderContext, leader, leaderView, subject)
         = AdequateLeaderFrozenWireOwnerIdentity(
             right, target, leaderContext,
             leader, leaderView, subject)
BY Isa
   DEF AdequateLeaderFrozenWireOwnerIdentity,
       AdequateLeaderFrozenWireOwnerIdentityFromCoordinates,
       AdequateLeaderFrozenWirePayloadIdentity

THEOREM AdequateLeaderServiceIdentityDeterminesFrozenWireOwnerIdentity ==
  \A left, right, target, leaderContext, leader, leaderView, subject:
    /\ AdequateLeaderFrozenTargetWireIdentity(
         left, target, leaderContext, leader, leaderView, subject)
    /\ AdequateLeaderFrozenTargetWireIdentity(
         right, target, leaderContext, leader, leaderView, subject)
    /\ AsyncLeaderWireServiceIdentity(left)
         = AsyncLeaderWireServiceIdentity(right)
    => AdequateLeaderFrozenWireOwnerIdentity(
         left, target, leaderContext, leader, leaderView, subject)
         =
       AdequateLeaderFrozenWireOwnerIdentity(
         right, target, leaderContext, leader, leaderView, subject)
BY AdequateLeaderFrozenWireRetryIdentityIsStable, Isa
   DEF AsyncLeaderWireServiceIdentity

THEOREM AdequateLeaderStableWireCompletionIsNotLiveService ==
  \A item, target, leaderContext, leader, leaderView, subject:
    /\ AdequateLeaderTargetWireIdentity(
         item, target, leaderContext, leader, leaderView, subject)
    /\ LeaderWireStableCompletionRecorded(item)
    => /\ LeaderWireServicedLifecycleRecorded(item)
       /\ LeaderWireTerminalLifecycleRecorded(item)
       /\ ~LeaderWireLogicalServiceActive(item)
       /\ AdequateLeaderFrozenWireOwnerIdentity(
            item, target, leaderContext, leader, leaderView, subject)
            \notin AdequateLeaderTargetLiveWireOwnerIdentitySet(
                     target, leaderContext, leader, leaderView, subject)
BY Isa
   DEF LeaderWireTerminalLifecycleRecorded,
       LeaderWireServicedLifecycleRecorded,
       LeaderWireLogicalServiceActive,
       LeaderWireStableCompletionRecorded,
       LeaderWireStableControlCompletion,
       AsyncControlServiceConsumed,
       AsyncControlServiceOccurrenceTombstoned,
       AdequateLeaderTargetLiveWireOwnerIdentitySet

\* Control service uses one fixed recipient/source/protocol-owner slot.  Once
\* the incumbent is consumed, a higher-view replacement keeps that slot
\* occupied and exits the frozen-view corridor.  Before abstract consumption
\* the newer packet remains behind the immutable predecessor.  The production
\* split may move both occurrences into the runtime only after the predecessor
\* owns the smaller lifecycle ordinal; FIFO, Busy-deferred, and effect-owner
\* selection then refines the same order.  A same/lower-view occurrence cannot
\* replace the slot.  Same-height recovery preserves the record, so an exact
\* consumed retry cannot resurrect.
THEOREM AdequateLeaderControlSlotCannotResurrectWhileCorridorPersists ==
  \A item, target, leaderContext, leader, leaderView, subject:
    /\ AdequateLeaderTargetWireIdentity(
         item, target, leaderContext, leader, leaderView, subject)
    /\ LeaderWireStableControlCompletion(item)
    /\ AdequateLeaderFrozenTargetCorridor(
         target, leaderContext, leader, leaderView)
    /\ AsyncNext
    /\ AdequateLeaderFrozenTargetCorridor(
         target, leaderContext, leader, leaderView)'
    /\ AdequateLeaderTargetWireIdentity(
         item, target, leaderContext, leader, leaderView, subject)'
    => /\ AsyncControlServiceOccurrenceTombstoned(item)'
       /\ ~LeaderWireLogicalServiceActive(item)'
BY AsyncControlServiceConsumedIdentityCannotReactivate, IsaT(180)
   DEF LeaderWireStableControlCompletion,
       LeaderWireTerminalLifecycleRecorded,
       LeaderWireLogicalServiceActive,
       AsyncControlServiceConsumed,
       AsyncControlServiceCurrentHeightItem,
       AsyncNext

\* Chunk completion is the existing bounded node/view/subject/chunk receipt.
\* The frozen corridor is post-GST, where responsive replay cannot clear it.
THEOREM AdequateLeaderChunkReceiptCannotResurrectWhileCorridorPersists ==
  \A item, target, leaderContext, leader, leaderView, subject:
    /\ AdequateLeaderTargetWireIdentity(
         item, target, leaderContext, leader, leaderView, subject)
    /\ item.kind = "Chunk"
    /\ LeaderWireStableCompletionRecorded(item)
    /\ AdequateLeaderFrozenTargetCorridor(
         target, leaderContext, leader, leaderView)
    /\ AsyncNext
    /\ AdequateLeaderFrozenTargetCorridor(
         target, leaderContext, leader, leaderView)'
    => LeaderWireStableCompletionRecorded(item)'
BY IsaT(300)
   DEF LeaderWireStableCompletionRecorded,
       AdequateLeaderFrozenTargetCorridor,
       AsyncNext, AsyncNonCrashStep,
       PreGstCrash, PreGstResponsiveCrash,
       PreGstResponsiveRestart, PreGstResponsiveReplay,
       ResetNodeSchedulerForRestart

\* Certified responses retain no independent history set.  Their bounded
\* request/claim lifecycle hands off to a candidate, and candidate service
\* reaches one of the durable body, higher-view, or Decision milestones below.
THEOREM AdequateLeaderCertifiedResponseCompletionCannotResurrect ==
  \A item, target, leaderContext, leader, leaderView, subject:
    /\ AdequateLeaderTargetWireIdentity(
         item, target, leaderContext, leader, leaderView, subject)
    /\ item.kind = "CertifiedResponse"
    /\ LeaderWireStableCertifiedResponseCompletion(item)
    /\ AdequateLeaderFrozenTargetCorridor(
         target, leaderContext, leader, leaderView)
    /\ AsyncNext
    /\ AdequateLeaderFrozenTargetCorridor(
         target, leaderContext, leader, leaderView)'
    => LeaderWireStableCertifiedResponseCompletion(item)'
BY IsaT(300)
   DEF LeaderWireStableCertifiedResponseCompletion,
       AdequateLeaderFrozenTargetCorridor,
       AsyncNext

LeaderWirePostGstServicedCarrier(item) ==
  /\ item \in AsyncNetworkItems
  /\ item.kind \in LeaderWireKinds
  /\ gst
  /\ (item.kind \in AsyncControlKinds
        => AsyncControlServiceCurrentHeightItem(item))
  /\ LeaderWireServicedLifecycleRecorded(item)

THEOREM LeaderWirePostGstServicedCarrierIsStepInvariant ==
  \A item \in AsyncNetworkItems:
    /\ LeaderWirePostGstServicedCarrier(item)
    /\ AsyncNext
    => LeaderWirePostGstServicedCarrier(item)'
BY AsyncControlServiceServicedIdentityCannotResurrect,
   GstAsyncStepIsMonotone, IsaT(300)
   DEF LeaderWirePostGstServicedCarrier,
       LeaderWireServicedLifecycleRecorded,
       LeaderWireStableCompletionRecorded,
       LeaderWireStableCertifiedResponseCompletion,
       AsyncControlServiceCurrentHeightItem,
       AsyncNext, AsyncNonCrashStep,
       PreGstCrash, PreGstResponsiveCrash,
       PreGstResponsiveRestart, PreGstResponsiveReplay,
       ResetNodeSchedulerForRestart

THEOREM AsyncSpecKeepsPostGstServicedWireLifecycle ==
  \A initialContext:
    AsyncSpecAt(initialContext)
      => [](\A item \in AsyncNetworkItems:
             LeaderWirePostGstServicedCarrier(item)
               => []LeaderWirePostGstServicedCarrier(item))
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE AsyncSpecAt(initialContext)
                 => [](\A item \in AsyncNetworkItems:
                        LeaderWirePostGstServicedCarrier(item)
                          => []LeaderWirePostGstServicedCarrier(item))
    <2>1. \A item \in AsyncNetworkItems:
             LeaderWirePostGstServicedCarrier(item)
               /\ [AsyncNext]_AsyncAllVars
             => LeaderWirePostGstServicedCarrier(item)'
      BY LeaderWirePostGstServicedCarrierIsStepInvariant, Isa
         DEF AsyncAllVars
    <2> QED BY <2>1, PTL DEF AsyncSpecAt
  <1> QED BY <1>1

THEOREM AdequateLeaderTerminalWireLifecycleCannotReactivate ==
  \A item, target, leaderContext, leader, leaderView, subject:
    /\ AdequateLeaderTargetWireIdentity(
         item, target, leaderContext, leader, leaderView, subject)
    /\ LeaderWireTerminalLifecycleRecorded(item)
    /\ AdequateLeaderFrozenTargetCorridor(
         target, leaderContext, leader, leaderView)
    /\ AsyncNext
    /\ AdequateLeaderFrozenTargetCorridor(
         target, leaderContext, leader, leaderView)'
    /\ AdequateLeaderTargetWireIdentity(
         item, target, leaderContext, leader, leaderView, subject)'
    => /\ LeaderWireTerminalLifecycleRecorded(item)'
       /\ ~LeaderWireLogicalServiceActive(item)'
BY AsyncControlServiceTombstoneCannotReactivate,
   AdequateLeaderChunkReceiptCannotResurrectWhileCorridorPersists,
   AdequateLeaderCertifiedResponseCompletionCannotResurrect,
   IsaT(240)
   DEF LeaderWireTerminalLifecycleRecorded,
       LeaderWireLogicalServiceActive,
       LeaderWireStableCompletionRecorded,
       AsyncControlServiceCurrentHeightItem,
       AsyncNext

AdequateLeaderServicedWireIdentityNoResurrectionProperty(
    specification) ==
  specification
    => [](\A item \in AsyncNetworkItems,
             target \in ValidatorIds,
             leaderContext \in ContextRecords,
             leader \in ValidatorIds,
             leaderView \in Views,
             subject \in Subjects:
            /\ AdequateLeaderTargetWireIdentity(
                 item, target, leaderContext,
                 leader, leaderView, subject)
            /\ LeaderWireStableCompletionRecorded(item)
            => [](
                 AdequateLeaderFrozenWireOwnerIdentity(
                   item, target, leaderContext,
                   leader, leaderView, subject)
                   \notin
                     AdequateLeaderTargetLiveWireOwnerIdentitySet(
                       target, leaderContext,
                       leader, leaderView, subject)))

THEOREM AsyncSpecProvidesServicedWireIdentityNoResurrection ==
  \A initialContext:
    AdequateLeaderServicedWireIdentityNoResurrectionProperty(
      AsyncSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE AdequateLeaderServicedWireIdentityNoResurrectionProperty(
                 AsyncSpecAt(initialContext))
    <2>1. AsyncSpecAt(initialContext)
             => [](\A item \in AsyncNetworkItems:
                    LeaderWirePostGstServicedCarrier(item)
                      => []LeaderWirePostGstServicedCarrier(item))
      BY AsyncSpecKeepsPostGstServicedWireLifecycle
    <2>2. \A item \in AsyncNetworkItems,
               target \in ValidatorIds,
               leaderContext \in ContextRecords,
               leader \in ValidatorIds,
               leaderView \in Views,
               subject \in Subjects:
             /\ AdequateLeaderTargetWireIdentity(
                  item, target, leaderContext,
                  leader, leaderView, subject)
             /\ LeaderWireStableCompletionRecorded(item)
             => LeaderWirePostGstServicedCarrier(item)
      BY Isa
         DEF AdequateLeaderTargetWireIdentity,
             AdequateLeaderFrozenTargetCorridor,
             LeaderWirePostGstServicedCarrier,
             LeaderWireServicedLifecycleRecorded,
             LeaderWireTerminalLifecycleRecorded,
             LeaderWireStableCompletionRecorded,
             AsyncControlServiceCurrentHeightItem,
             LeaderWireCarriesContext
    <2>3. \A item \in AsyncNetworkItems,
               target \in ValidatorIds,
               leaderContext \in ContextRecords,
               leader \in ValidatorIds,
               leaderView \in Views,
               subject \in Subjects:
             /\ LeaderWirePostGstServicedCarrier(item)
             /\ AdequateLeaderFrozenTargetWireIdentity(
                  item, target, leaderContext,
                  leader, leaderView, subject)
             => AdequateLeaderFrozenWireOwnerIdentity(
                  item, target, leaderContext,
                  leader, leaderView, subject)
                  \notin
                    AdequateLeaderTargetLiveWireOwnerIdentitySet(
                      target, leaderContext,
                      leader, leaderView, subject)
      BY IsaT(240)
         DEF LeaderWirePostGstServicedCarrier,
             LeaderWireServicedLifecycleRecorded,
             LeaderWireTerminalLifecycleRecorded,
             LeaderWireLogicalServiceActive,
             AdequateLeaderFrozenTargetWireIdentity,
             AdequateLeaderTargetLiveWireOwnerIdentitySet
    <2> QED BY <2>1, <2>2, <2>3, PTL
         DEF AdequateLeaderServicedWireIdentityNoResurrectionProperty,
             AdequateLeaderTargetWireIdentity
  <1> QED BY <1>1

THEOREM AdequateLeaderStableWireCompletionCannotResurrect ==
  \A item, target, leaderContext, leader, leaderView, subject:
    /\ AdequateLeaderTargetWireIdentity(
         item, target, leaderContext, leader, leaderView, subject)
    /\ LeaderWireStableCompletionRecorded(item)
    /\ AdequateLeaderFrozenTargetCorridor(
         target, leaderContext, leader, leaderView)
    /\ AsyncNext
    /\ AdequateLeaderFrozenTargetCorridor(
         target, leaderContext, leader, leaderView)'
    /\ AdequateLeaderTargetWireIdentity(
         item, target, leaderContext, leader, leaderView, subject)'
    => /\ AdequateLeaderFrozenWireOwnerIdentity(
             item, target, leaderContext, leader, leaderView, subject)
             \notin AdequateLeaderTargetLiveWireOwnerIdentitySet(
                     target, leaderContext, leader, leaderView, subject)
       /\ AdequateLeaderFrozenWireOwnerIdentity(
             item, target, leaderContext, leader, leaderView, subject)
             \notin AdequateLeaderTargetLiveWireOwnerIdentitySet(
                     target, leaderContext, leader, leaderView, subject)'
BY AdequateLeaderTerminalWireLifecycleCannotReactivate,
   AdequateLeaderStableWireCompletionIsNotLiveService, IsaT(180)
   DEF LeaderWireStableCompletionRecorded,
       LeaderWireTerminalLifecycleRecorded,
       LeaderWireLogicalServiceActive,
       AdequateLeaderTargetLiveWireOwnerIdentitySet

\* A locally formed CommitQC is first persisted by the forming leader.  That
\* non-target PersistDecision is not terminal for `target`; its only accepted
\* target-corridor outcome is the exact rebroadcast/transport handoff.
AdequateLeaderTargetCommitQcRebroadcastResidual(
    target, leaderContext, leader, leaderView, subject) ==
  /\ target # leader
  /\ AdequateLeaderFrozenTargetCorridor(
       target, leaderContext, leader, leaderView)
  /\ subject \in Subjects
  /\ \E candidate \in AsyncCandidateSet:
       /\ ExactLeaderCurrentRankWitness(
            candidate, DecisionSemanticRank(2), leaderContext,
            leader, leaderView, subject)
       /\ candidate.kind = "PersistDecision"
       /\ \E request \in PersistDecisionRequests(candidate):
            request.rebroadcast

AdequateLeaderTargetDueTransportResidual(
    target, leaderContext, leader, leaderView, subject) ==
  \E packet \in OverdueResponsivePackets:
    /\ AdequateLeaderTargetWireIdentity(
         packet.item, target, leaderContext, leader, leaderView, subject)
    /\ LeaderWireDueTransportResidual(packet)

AdequateLeaderTargetRunnerAdmissionResidual(
    target, leaderContext, leader, leaderView, subject) ==
  \E item \in AsyncNetworkItems:
    /\ AdequateLeaderTargetWireIdentity(
         item, target, leaderContext, leader, leaderView, subject)
    /\ LeaderWireRunnerAdmissionResidual(item)

AdequateLeaderTargetCertifiedResponseCapacityResidual(
    target, leaderContext, leader, leaderView, subject) ==
  \E item \in AsyncNetworkItems:
    /\ AdequateLeaderTargetWireIdentity(
         item, target, leaderContext, leader, leaderView, subject)
    /\ CertifiedResponsePhysicalCompletionDebtResidual(item)

\* Before any fixed-subject owner exists, the adequate leader's exact
\* proposal selector is the only admissible source of a new corridor
\* subject.  This prevents the producer residual from manufacturing an
\* arbitrary member of `Subjects` merely to satisfy an existential entry.
AdequateLeaderTargetProtocolSubjectSource(
    target, leaderContext, leader, leaderView, subject) ==
  /\ AdequateLeaderFrozenTargetCorridor(
       target, leaderContext, leader, leaderView)
  /\ subject \in Subjects
  /\ subject = AsyncProposalSubject(leader)

\* A control occurrence which names another subject is scheduler drain, not
\* evidence that the fixed target protocol advanced.  Bind the endpoint to
\* the exact reconstructed delivery candidate and its frozen semantic phase;
\* an unrelated control slot cannot retire this occurrence.
AdequateLeaderTargetOffSubjectControlOccurrenceIdentity(
    item, target, leaderContext, leader, leaderView,
    subject, occurrenceRank) ==
  /\ occurrenceRank \in AdequateLeaderTargetOccurrenceRankCarrier
  /\ item \in AsyncNetworkItems
  /\ item.kind \in AsyncControlKinds
  /\ AdequateLeaderTargetWireIdentity(
       item, target, leaderContext, leader, leaderView, subject)
  /\ AdequateLeaderFrozenTargetCandidateIdentity(
       DeliveryCandidate(item), occurrenceRank[1],
       target, leaderContext, leader, leaderView, subject)
  /\ ~AdequateLeaderTargetProtocolSubjectSource(
       target, leaderContext, leader, leaderView, subject)

AdequateLeaderTargetSameOrLowerControlRetry(item, retry) ==
  /\ retry \in AsyncNetworkItems
  /\ retry.kind \in AsyncControlKinds
  /\ AsyncControlServiceSlot(
       retry.envelope.recipient, retry.source, retry.kind)
       = AsyncControlServiceSlot(
           item.envelope.recipient, item.source, item.kind)
  /\ AsyncControlServiceCurrentHeightItem(retry)
  /\ AsyncControlServiceRecordForItem(retry).context = context
  /\ AsyncControlServiceRecordForItem(retry).height = height
  /\ AsyncControlItemView(retry)
       <= AsyncControlServiceRecordForItem(retry).view

AdequateLeaderTargetSameOrLowerControlRetriesAdmissionBlocked(item) ==
  \A retry \in AsyncNetworkItems:
    AdequateLeaderTargetSameOrLowerControlRetry(item, retry)
      => /\ AsyncControlServiceAdmissionCoalesced(retry)
         /\ ~AsyncControlServiceAdmissionStartsOrReplaces(retry)
         /\ ~CanAdmitIngressItem(retry)

THEOREM AdequateLeaderCurrentControlOwnerBlocksSameOrLowerRetries ==
  \A item \in AsyncNetworkItems:
    /\ item.kind \in AsyncControlKinds
    /\ AsyncControlServiceOccurrenceIsCurrentOwner(item)
    => AdequateLeaderTargetSameOrLowerControlRetriesAdmissionBlocked(item)
BY AsyncControlServiceSameOrLowerViewCannotReplace, Isa
   DEF AdequateLeaderTargetSameOrLowerControlRetriesAdmissionBlocked,
       AdequateLeaderTargetSameOrLowerControlRetry,
       AsyncControlServiceOccurrenceIsCurrentOwner,
       CanAdmitIngressItem

\* Physical retransmission may place another packet on the network after the
\* exact reducer candidate drains.  The safety fact required by the outer
\* episode is only
\* that the old reducer candidate cannot return: either the exact unconsumed
\* slot remains the current coalescing owner, or the slot has advanced and the
\* old identity is durably serviced/advanced.  This memory deliberately says
\* nothing about simultaneous absence of every same/lower physical packet;
\* Byzantine senders may rotate the finite packet variants forever, but each
\* variant still fails the retained slot's admission gate.
AdequateLeaderTargetOffSubjectControlCandidateOwnerIdentity(
    item, target, leaderContext, leader, leaderView,
    subject, occurrenceRank) ==
  AdequateLeaderFrozenCandidateOwnerIdentity(
    DeliveryCandidate(item), occurrenceRank[1],
    target, leaderContext, leader, leaderView, subject)

AdequateLeaderTargetOffSubjectControlRetirementMemory(
    item, target, leaderContext, leader, leaderView,
    subject, occurrenceRank) ==
  /\ occurrenceRank \in AdequateLeaderTargetOccurrenceRankCarrier
  /\ AdequateLeaderFrozenTargetWireIdentity(
       item, target, leaderContext, leader, leaderView, subject)
  /\ AdequateLeaderTargetOffSubjectControlCandidateOwnerIdentity(
       item, target, leaderContext, leader, leaderView,
       subject, occurrenceRank)
       \notin
         AdequateLeaderTargetLiveCandidateOwnerIdentitySet(
           target, leaderContext, leader, leaderView, subject)
  /\ \/ AsyncControlServiceOccurrenceIsCurrentOwner(item)
     \/ AsyncControlServiceIdentityServicedOrAdvanced(item)

AdequateLeaderTargetOffSubjectControlRetirementClosed(
    item, target, leaderContext, leader, leaderView,
    subject, occurrenceRank) ==
  AdequateLeaderTargetOffSubjectControlRetirementMemory(
    item, target, leaderContext, leader, leaderView,
    subject, occurrenceRank)

AdequateLeaderTargetOffSubjectControlClosedOwnerIdentitySet(
    target, leaderContext, leader, leaderView,
    subject, occurrenceRank) ==
  LET closedItems ==
        {item \in AsyncNetworkItems:
           /\ AdequateLeaderTargetOffSubjectControlOccurrenceIdentity(
                item, target, leaderContext, leader, leaderView,
                subject, occurrenceRank)
           /\ AdequateLeaderTargetOffSubjectControlRetirementClosed(
                item, target, leaderContext,
                leader, leaderView, subject, occurrenceRank)}
  IN {AdequateLeaderFrozenWireOwnerIdentity(
        item, target, leaderContext, leader, leaderView, subject):
        item \in closedItems}
       \cup
     {AdequateLeaderTargetOffSubjectControlCandidateOwnerIdentity(
        item, target, leaderContext, leader, leaderView,
        subject, occurrenceRank):
        item \in closedItems}

THEOREM AdequateLeaderOffSubjectControlRetirementMemoryIsStepInvariant ==
  \A item, target, leaderContext, leader, leaderView,
     subject, occurrenceRank:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateServiceTombstoneLifecycleInvariant
    /\ gst
    /\ AdequateLeaderTargetOffSubjectControlRetirementMemory(
         item, target, leaderContext, leader, leaderView,
         subject, occurrenceRank)
    /\ [AsyncNext]_AsyncAllVars
    => AdequateLeaderTargetOffSubjectControlRetirementClosed(
         item, target, leaderContext, leader, leaderView,
         subject, occurrenceRank)'
BY AsyncControlServiceSameOrLowerViewCannotReplace,
   AsyncControlServiceServicedIdentityCannotResurrect,
   AsyncControlServiceTombstoneCannotReactivate,
   AsyncRetiredControlServiceAdmissionDropsWithoutCandidate,
   IsaT(900)
   DEF AdequateLeaderTargetOffSubjectControlRetirementMemory,
       AdequateLeaderTargetOffSubjectControlRetirementClosed,
       AdequateLeaderTargetOffSubjectControlCandidateOwnerIdentity,
       AdequateLeaderTargetLiveCandidateOwnerIdentitySet,
       AdequateLeaderTargetRankOwnerIdentitySet,
       AdequateLeaderTargetRankOwnerSet,
       AdequateLeaderTargetSameOrLowerControlRetriesAdmissionBlocked,
       AdequateLeaderTargetSameOrLowerControlRetry,
       AsyncControlServiceOccurrenceIsCurrentOwner,
       AsyncControlServiceIdentityServicedOrAdvanced,
       AsyncControlServiceOccurrenceTombstoned,
       AsyncControlServiceSlotTransition,
       AsyncControlServiceAdmissionStartsOrReplaces,
       AsyncControlServiceAdmissionCoalesced,
       AsyncControlServiceStrictlyNewerItem,
       CandidateAdmissionCoalesced, FreshCandidateSequence,
       IngressPacketPolicyRejected, CanAdmitIngressItem,
       AsyncNext, AsyncAllVars

AdequateLeaderTargetOffSubjectControlNoReentryProperty(specification) ==
  specification
    => [](\A item \in AsyncNetworkItems,
             target \in ValidatorIds,
             leaderContext \in ContextRecords,
             leader \in ValidatorIds,
             leaderView \in Views,
             subject \in Subjects,
             occurrenceRank \in
               AdequateLeaderTargetOccurrenceRankCarrier:
            /\ gst
            /\ AdequateLeaderTargetOffSubjectControlOccurrenceIdentity(
                 item, target, leaderContext, leader, leaderView,
                 subject, occurrenceRank)
            /\ AdequateLeaderTargetOffSubjectControlRetirementMemory(
                 item, target, leaderContext,
                 leader, leaderView, subject, occurrenceRank)
              => []AdequateLeaderTargetOffSubjectControlRetirementClosed(
                    item, target, leaderContext,
                    leader, leaderView, subject, occurrenceRank))

\* This last arm is the exact fixed-corridor producer debt: no ranked owner,
\* rebroadcast owner, due packet, ingress owner, or certified-response
\* capacity owner currently carries the frozen target/leader/view/subject.
\* It includes a not-yet-due packet and a missing proposal/vote/QC producer;
\* those cases require separate time/producer arguments before this residual
\* can be discharged.
AdequateLeaderTargetProducerResidual(
    target, leaderContext, leader, leaderView, subject) ==
  /\ AdequateLeaderTargetProtocolSubjectSource(
       target, leaderContext, leader, leaderView, subject)
  /\ ~(\E rank \in AdequateLeaderTargetSemanticRankCarrier:
         AdequateLeaderTargetRankFrontier(
           target, leaderContext, leader, leaderView, subject, rank))
  /\ ~AdequateLeaderTargetCommitQcRebroadcastResidual(
       target, leaderContext, leader, leaderView, subject)
  /\ ~AdequateLeaderTargetDueTransportResidual(
       target, leaderContext, leader, leaderView, subject)
  /\ ~AdequateLeaderTargetRunnerAdmissionResidual(
       target, leaderContext, leader, leaderView, subject)
  /\ ~AdequateLeaderTargetCertifiedResponseCapacityResidual(
       target, leaderContext, leader, leaderView, subject)

AdequateLeaderTargetProducerTransportResidual(
    target, leaderContext, leader, leaderView, subject) ==
  \/ AdequateLeaderTargetCommitQcRebroadcastResidual(
       target, leaderContext, leader, leaderView, subject)
  \/ AdequateLeaderTargetDueTransportResidual(
       target, leaderContext, leader, leaderView, subject)
  \/ AdequateLeaderTargetRunnerAdmissionResidual(
       target, leaderContext, leader, leaderView, subject)
  \/ AdequateLeaderTargetCertifiedResponseCapacityResidual(
       target, leaderContext, leader, leaderView, subject)
  \/ AdequateLeaderTargetProducerResidual(
       target, leaderContext, leader, leaderView, subject)

\* Servicing a concrete owner has three disjoint outcomes: Decision/strict
\* occurrence descent, an equal-count identity replacement, or a
\* count-increasing replenishment.  Only the first is progress.  The other
\* two enter the separate finite non-descent episode below.
AdequateLeaderTargetEqualCountOwnerReplacementAction(
    target, leaderContext, leader, leaderView, subject, rank) ==
  /\ rank \in AdequateLeaderTargetSemanticRankCarrier
  /\ subject \in Subjects
  /\ AdequateLeaderFrozenTargetCorridor(
       target, leaderContext, leader, leaderView)
  /\ IsFiniteSet(
       AdequateLeaderTargetRankOwnerSet(
         target, leaderContext, leader, leaderView, subject, rank))
  /\ IsFiniteSet(
       AdequateLeaderTargetRankOwnerSet(
         target, leaderContext, leader, leaderView, subject, rank)')
  /\ AsyncNext
  /\ ~NodeHasDecision(target)'
  /\ AdequateLeaderTargetRankOwnerCount(
       target, leaderContext, leader, leaderView, subject, rank)'
       = AdequateLeaderTargetRankOwnerCount(
           target, leaderContext, leader, leaderView, subject, rank)
  /\ AdequateLeaderTargetRankOwnerIdentitySet(
       target, leaderContext, leader, leaderView, subject, rank)'
       # AdequateLeaderTargetRankOwnerIdentitySet(
           target, leaderContext, leader, leaderView, subject, rank)

AdequateLeaderTargetCountIncreasingReplenishmentAction(
    target, leaderContext, leader, leaderView, subject, rank) ==
  /\ rank \in AdequateLeaderTargetSemanticRankCarrier
  /\ subject \in Subjects
  /\ AdequateLeaderFrozenTargetCorridor(
       target, leaderContext, leader, leaderView)
  /\ IsFiniteSet(
       AdequateLeaderTargetRankOwnerSet(
         target, leaderContext, leader, leaderView, subject, rank))
  /\ IsFiniteSet(
       AdequateLeaderTargetRankOwnerSet(
         target, leaderContext, leader, leaderView, subject, rank)')
  /\ AsyncNext
  /\ ~NodeHasDecision(target)'
  /\ AdequateLeaderTargetRankOwnerCount(
       target, leaderContext, leader, leaderView, subject, rank)'
       > AdequateLeaderTargetRankOwnerCount(
           target, leaderContext, leader, leaderView, subject, rank)

AdequateLeaderTargetRankIntroducedOwnerIdentitySet(
    target, leaderContext, leader, leaderView, subject, rank) ==
  AdequateLeaderTargetRankOwnerIdentitySet(
    target, leaderContext, leader, leaderView, subject, rank)'
    \
  AdequateLeaderTargetRankOwnerIdentitySet(
    target, leaderContext, leader, leaderView, subject, rank)

AdequateLeaderTargetRankRetiredOwnerIdentitySet(
    target, leaderContext, leader, leaderView, subject, rank) ==
  AdequateLeaderTargetRankOwnerIdentitySet(
    target, leaderContext, leader, leaderView, subject, rank)
    \
  AdequateLeaderTargetRankOwnerIdentitySet(
    target, leaderContext, leader, leaderView, subject, rank)'

THEOREM AdequateLeaderTargetEqualCountReplacementIntroducesAndRetires ==
  \A target, leaderContext, leader, leaderView, subject, rank:
    AdequateLeaderTargetEqualCountOwnerReplacementAction(
      target, leaderContext, leader, leaderView, subject, rank)
      => /\ AdequateLeaderTargetRankIntroducedOwnerIdentitySet(
               target, leaderContext, leader,
               leaderView, subject, rank) # {}
         /\ AdequateLeaderTargetRankRetiredOwnerIdentitySet(
               target, leaderContext, leader,
               leaderView, subject, rank) # {}
BY FS_Image, FS_CardinalityType, IsaT(180)
   DEF AdequateLeaderTargetEqualCountOwnerReplacementAction,
       AdequateLeaderTargetRankOwnerCount,
       AdequateLeaderTargetRankOwnerIdentitySet,
       AdequateLeaderTargetRankIntroducedOwnerIdentitySet,
       AdequateLeaderTargetRankRetiredOwnerIdentitySet

THEOREM AdequateLeaderTargetCountIncreaseIntroducesOwnerIdentity ==
  \A target, leaderContext, leader, leaderView, subject, rank:
    AdequateLeaderTargetCountIncreasingReplenishmentAction(
      target, leaderContext, leader, leaderView, subject, rank)
      => AdequateLeaderTargetRankIntroducedOwnerIdentitySet(
           target, leaderContext, leader,
           leaderView, subject, rank) # {}
BY FS_Image, FS_CardinalityType, IsaT(180)
   DEF AdequateLeaderTargetCountIncreasingReplenishmentAction,
       AdequateLeaderTargetRankOwnerCount,
       AdequateLeaderTargetRankOwnerIdentitySet,
       AdequateLeaderTargetRankIntroducedOwnerIdentitySet

AdequateLeaderTargetRankReplenishmentAction(
    target, leaderContext, leader, leaderView, subject, rank) ==
  AdequateLeaderTargetCountIncreasingReplenishmentAction(
    target, leaderContext, leader, leaderView, subject, rank)

AdequateLeaderTargetRankReplenishmentResidual(
    target, leaderContext, leader, leaderView, subject, rank) ==
  /\ AdequateLeaderTargetRankFrontier(
       target, leaderContext, leader, leaderView, subject, rank)
  /\ ENABLED
       <<AdequateLeaderTargetRankReplenishmentAction(
           target, leaderContext, leader, leaderView,
           subject, rank)>>_AsyncAllVars

AdequateLeaderTargetStrictOccurrenceDescentGoal(
    target, leaderContext, leader, leaderView, subject, occurrenceRank) ==
  \/ NodeHasDecision(target)
  \/ \E lowerOccurrenceRank \in
       SetLessThan(
         occurrenceRank,
         AdequateLeaderTargetOccurrenceRankOrdering,
         AdequateLeaderTargetOccurrenceRankCarrier):
       AdequateLeaderTargetOccurrenceRankFrontier(
         target, leaderContext, leader,
         leaderView, subject, lowerOccurrenceRank)

\* The well-founded Decision induction may recurse only from a lower frontier
\* which is still the leader's current deterministic protocol subject.  An
\* off-subject lower count is scheduler drain and is handled by the outer
\* subject-switch budget instead.
AdequateLeaderTargetProductiveStrictOccurrenceDescentGoal(
    target, leaderContext, leader, leaderView, subject, occurrenceRank) ==
  \/ NodeHasDecision(target)
  \/ /\ AdequateLeaderTargetProtocolSubjectSource(
          target, leaderContext, leader, leaderView, subject)
     /\ \E lowerOccurrenceRank \in
          SetLessThan(
            occurrenceRank,
            AdequateLeaderTargetOccurrenceRankOrdering,
            AdequateLeaderTargetOccurrenceRankCarrier):
          AdequateLeaderTargetOccurrenceRankFrontier(
            target, leaderContext, leader,
            leaderView, subject, lowerOccurrenceRank)

AdequateLeaderTargetDecisionOrStrictlyLowerOccurrenceAction(
    target, leaderContext, leader, leaderView, subject, occurrenceRank) ==
  /\ AdequateLeaderTargetOccurrenceRankFrontier(
       target, leaderContext, leader,
       leaderView, subject, occurrenceRank)
  /\ AsyncNext
  /\ AdequateLeaderTargetStrictOccurrenceDescentGoal(
       target, leaderContext, leader,
       leaderView, subject, occurrenceRank)'

(***************************************************************************
The source occurrence is frozen across a producer/transport handoff.

`AdequateLeaderTargetSameOrHigherOccurrenceFrontier` contains exactly the
non-progress occurrence ranks for one semantic phase: equal-count owner
replacement preserves the second coordinate and replenishment increases it.
A lower count or lower semantic phase is already the strict goal.  A producer
corridor may be entered only after no same-or-higher candidate frontier
remains.  Consequently the rank-indexed producer closure below may terminate
only at Decision/strict descent; returning an arbitrary occurrence rank would
reintroduce the producer lasso.
***************************************************************************)
AdequateLeaderTargetSameOrHigherOccurrenceFrontier(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank) ==
  /\ sourceOccurrenceRank \in
       AdequateLeaderTargetOccurrenceRankCarrier
  /\ \E currentOccurrenceRank \in
       AdequateLeaderTargetOccurrenceRankCarrier:
       /\ currentOccurrenceRank[1] = sourceOccurrenceRank[1]
       /\ sourceOccurrenceRank[2] <= currentOccurrenceRank[2]
       /\ AdequateLeaderTargetOccurrenceRankFrontier(
            target, leaderContext, leader, leaderView,
            subject, currentOccurrenceRank)

AdequateLeaderTargetProducerTransportResidualAtOccurrence(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank) ==
  /\ sourceOccurrenceRank \in
       AdequateLeaderTargetOccurrenceRankCarrier
  /\ ~AdequateLeaderTargetStrictOccurrenceDescentGoal(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank)
  /\ ~AdequateLeaderTargetSameOrHigherOccurrenceFrontier(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank)
  /\ AdequateLeaderTargetProducerTransportResidual(
       target, leaderContext, leader, leaderView, subject)

AdequateLeaderTargetOccurrenceEpisodeActive(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank) ==
  \/ AdequateLeaderTargetSameOrHigherOccurrenceFrontier(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank)
  \/ AdequateLeaderTargetProducerTransportResidualAtOccurrence(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank)

AdequateLeaderTargetNonDescentEpisodeAction(
    target, leaderContext, leader, leaderView, subject, rank) ==
  \/ AdequateLeaderTargetEqualCountOwnerReplacementAction(
       target, leaderContext, leader, leaderView, subject, rank)
  \/ AdequateLeaderTargetCountIncreasingReplenishmentAction(
       target, leaderContext, leader, leaderView, subject, rank)

(***************************************************************************
The known set is frozen at the start of one service episode.  It must contain
exactly the live logical owners then present; a later state is a genuine
non-descent discovery only when it exposes an identity outside that frozen
set.  This makes the residual false in its source state.  In particular it is
not `ENABLED NonDescentAction`, which was already true before any service and
made the old leads-to tautological.

The remaining budget is the complement of `known` in the immutable finite
owner universe.  Equal replacement and replenishment may consume that budget
by discovery, but neither is itself a progress goal.
***************************************************************************)
AdequateLeaderTargetEpisodeKnownOwnerSet(
    target, leaderContext, leader, leaderView, subject, known) ==
  /\ IsFiniteSet(known)
  /\ known \subseteq
       AdequateLeaderFrozenOwnerUniverse(
         target, leaderContext, leader, leaderView, subject)

AdequateLeaderTargetEpisodeStartsWithCurrentOwners(
    target, leaderContext, leader, leaderView, subject, known) ==
  /\ AdequateLeaderTargetEpisodeKnownOwnerSet(
       target, leaderContext, leader, leaderView, subject, known)
  /\ known =
       AdequateLeaderTargetLiveOwnerIdentitySet(
         target, leaderContext, leader, leaderView, subject)

AdequateLeaderTargetNonDescentDiscoveredOwnerIdentitySet(
    target, leaderContext, leader, leaderView, subject, known) ==
  AdequateLeaderTargetLiveOwnerIdentitySet(
    target, leaderContext, leader, leaderView, subject)
    \ known

AdequateLeaderTargetNonDescentEpisodeBudget(
    target, leaderContext, leader, leaderView, subject, known) ==
  Cardinality(
    AdequateLeaderFrozenOwnerUniverse(
      target, leaderContext, leader, leaderView, subject)
      \ known)

AdequateLeaderTargetOwnerIdentityRetirementAction(
    target, leaderContext, leader, leaderView,
    subject, occurrenceRank, identity) ==
  /\ AdequateLeaderTargetOccurrenceRankFrontier(
       target, leaderContext, leader,
       leaderView, subject, occurrenceRank)
  /\ identity \in
       AdequateLeaderTargetLiveOwnerIdentitySet(
         target, leaderContext, leader, leaderView, subject)
  /\ AsyncNext
  /\ ~AdequateLeaderTargetStrictOccurrenceDescentGoal(
       target, leaderContext, leader,
       leaderView, subject, occurrenceRank)'
  /\ identity \notin
       AdequateLeaderTargetLiveOwnerIdentitySet(
         target, leaderContext, leader, leaderView, subject)'

(***************************************************************************
Candidate lifecycle closure.

`AsyncCandidateServiceIdentity` is the transition-level durable key.  It
retains the candidate's frozen context/height, semantic view, derived leader,
subject, work kind, local owner, and the same route-neutral immutable
`{class, workKind, causalOrigin, item, evidence, body, manifest, commitment}`
payload used by `AdequateLeaderFrozenCandidateOwnerIdentity`.  Consumer
view/generation are deliberately absent, so same-height restart is the same
logical occurrence.

Successful FIFO or Busy-deferred service retains a transient marker through
the complete same-generation/view episode.  A same-origin successor or a
different causal origin's monotone Core fact cannot replace that identity:
doing so admits the A -> B -> A replenishment lasso.  A proofless ignored
occurrence gets no marker: the exact BodyAvailable fetch case is covered by
the monotone preflight retirement, while every other such occurrence remains
an explicit finite frozen-identity episode.  Only an eligible internal no-item
discard retains a restart-stable terminal tombstone.  Both bounded record
classes reuse the immutable lifecycle-slot and finite work-kind carrier proved
in the source model.
***************************************************************************)
AdequateLeaderTargetCandidateOwnerIdentityRetirementAction(
    target, leaderContext, leader, leaderView,
    subject, occurrenceRank, identity) ==
  /\ AdequateLeaderTargetOwnerIdentityRetirementAction(
       target, leaderContext, leader, leaderView,
       subject, occurrenceRank, identity)
  /\ identity \in
       AdequateLeaderTargetLiveCandidateOwnerIdentitySet(
         target, leaderContext, leader, leaderView, subject)

AdequateLeaderTargetCandidateOwnerIdentityWitness(
    target, leaderContext, leader, leaderView,
    subject, identity, candidate, rank) ==
  /\ candidate \in AsyncCandidateSet
  /\ rank \in AdequateLeaderTargetSemanticRankCarrier
  /\ AdequateLeaderFrozenTargetCandidateIdentity(
       candidate, rank, target, leaderContext,
       leader, leaderView, subject)
  /\ identity =
       AdequateLeaderFrozenCandidateOwnerIdentity(
         candidate, rank, target, leaderContext,
         leader, leaderView, subject)

\* Successful service and terminal discard are deliberately separate action
\* predicates.  The former installs process-generation serviced memory; the
\* latter installs restart-stable serviced memory.  Binding the retired frozen
\* identity to the concrete candidate in the action prevents a singleton
\* service elsewhere in the same step from being used as evidence for this
\* identity.
AdequateLeaderTargetCandidateSuccessfulServiceRetirementAction(
    target, leaderContext, leader, leaderView,
    subject, occurrenceRank, identity) ==
  /\ AdequateLeaderTargetCandidateOwnerIdentityRetirementAction(
       target, leaderContext, leader, leaderView,
       subject, occurrenceRank, identity)
  /\ \E candidate \in AsyncCandidateSet,
        rank \in AdequateLeaderTargetSemanticRankCarrier:
       /\ AdequateLeaderTargetCandidateOwnerIdentityWitness(
            target, leaderContext, leader, leaderView,
            subject, identity, candidate, rank)
       /\ AsyncCandidateServicesThisStep = {candidate}
       /\ AsyncCandidateServiceEligibleAfterStep(candidate)
       /\ AsyncControlServiceSlotTransition

AdequateLeaderTargetCandidateTerminalDiscardRetirementAction(
    target, leaderContext, leader, leaderView,
    subject, occurrenceRank, identity) ==
  /\ AdequateLeaderTargetCandidateOwnerIdentityRetirementAction(
       target, leaderContext, leader, leaderView,
       subject, occurrenceRank, identity)
  /\ \E candidate \in AsyncCandidateSet,
        rank \in AdequateLeaderTargetSemanticRankCarrier:
       /\ AdequateLeaderTargetCandidateOwnerIdentityWitness(
            target, leaderContext, leader, leaderView,
            subject, identity, candidate, rank)
       /\ AsyncCandidateTerminallyDiscardedThisStep(candidate)
       /\ AsyncCandidateTerminalRetirementEligibleAfterStep(candidate)

AdequateLeaderTargetCandidateServicedRetirementAction(
    target, leaderContext, leader, leaderView,
    subject, occurrenceRank, identity) ==
  \/ AdequateLeaderTargetCandidateSuccessfulServiceRetirementAction(
       target, leaderContext, leader, leaderView,
       subject, occurrenceRank, identity)
  \/ AdequateLeaderTargetCandidateTerminalDiscardRetirementAction(
       target, leaderContext, leader, leaderView,
       subject, occurrenceRank, identity)

AdequateLeaderFrozenNetworkCandidateServiceIdentityFromPayload(
    payload, owner, leaderContext, leader, leaderView, subject, workKind) ==
  [target |-> owner,
   context |-> leaderContext,
   height |-> leaderContext.height,
   leader |-> leader,
   view |-> leaderView,
   subject |-> subject,
   phase |-> workKind,
   owner |-> owner,
   kind |-> "Candidate",
   payload |-> payload]

AdequateLeaderFrozenNetworkCandidateServiceIdentityCarrier(
    target, leaderContext, leader, leaderView, subject) ==
  {AdequateLeaderFrozenNetworkCandidateServiceIdentityFromPayload(
     payload, owner, leaderContext, leader, leaderView, subject, workKind):
     payload \in
       AdequateLeaderFrozenCandidatePayloadCarrier(
         target, leaderContext, leader, leaderView, subject),
     owner \in {target, leader},
     workKind \in AsyncWorkKinds}

AdequateLeaderCandidateServiceTombstones(
    target, leaderContext, leader, leaderView, subject) ==
  {record \in AsyncCandidateServiceTombstones:
     record.identity \in
       AdequateLeaderFrozenNetworkCandidateServiceIdentityCarrier(
         target, leaderContext, leader, leaderView, subject)}

THEOREM AdequateLeaderNetworkCandidateServicePayloadMatchesImmutable ==
  \A candidate \in AsyncCandidateSet:
    AsyncCandidateServicePayload(candidate)
      = AdequateLeaderImmutableCandidatePayload(candidate)
BY Isa
   DEF AsyncCandidateServicePayload,
       AsyncRouteNeutralCandidateItem,
       AsyncRouteNeutralCandidateEvidence,
       AdequateLeaderImmutableCandidatePayload,
       AdequateLeaderRouteNeutralCandidateItem,
       AdequateLeaderRouteNeutralCandidateEvidence

THEOREM AdequateLeaderOwnerIdentityDeterminesNetworkServiceIdentity ==
  \A left, right, rank, target, leaderContext,
     leader, leaderView, subject:
    /\ left \in AsyncCandidateSet
    /\ right \in AsyncCandidateSet
    /\ AdequateLeaderFrozenTargetCandidateIdentity(
         left, rank, target, leaderContext,
         leader, leaderView, subject)
    /\ AdequateLeaderFrozenTargetCandidateIdentity(
         right, rank, target, leaderContext,
         leader, leaderView, subject)
    /\ AdequateLeaderFrozenCandidateOwnerIdentity(
         left, rank, target, leaderContext,
         leader, leaderView, subject)
         =
       AdequateLeaderFrozenCandidateOwnerIdentity(
         right, rank, target, leaderContext,
         leader, leaderView, subject)
    => AsyncCandidateServiceIdentity(left)
         = AsyncCandidateServiceIdentity(right)
BY AdequateLeaderFrozenCandidateOwnerIdentityIsInjective,
   AdequateLeaderNetworkCandidateServicePayloadMatchesImmutable,
   IsaT(300)
   DEF AdequateLeaderFrozenTargetCandidateIdentity,
       ExactLeaderFrozenSemanticIdentity,
       AsyncCandidateServiceIdentity,
       AsyncCandidateServicePayload,
       AdequateLeaderImmutableCandidatePayload

THEOREM AdequateLeaderCandidateServiceIdentityIsInFrozenCarrier ==
  \A candidate, rank, target, leaderContext,
     leader, leaderView, subject:
    /\ candidate \in AsyncCandidateSet
    /\ AdequateLeaderFrozenTargetCorridor(
         target, leaderContext, leader, leaderView)
    /\ AdequateLeaderFrozenTargetCandidateIdentity(
         candidate, rank, target, leaderContext,
         leader, leaderView, subject)
    => AsyncCandidateServiceIdentity(candidate)
         \in AdequateLeaderFrozenNetworkCandidateServiceIdentityCarrier(
              target, leaderContext, leader, leaderView, subject)
BY AdequateLeaderFrozenTargetCandidatePayloadIsInStaticCarrier,
   AdequateLeaderNetworkCandidateServicePayloadMatchesImmutable,
   IsaT(300)
   DEF AdequateLeaderFrozenNetworkCandidateServiceIdentityCarrier,
       AdequateLeaderFrozenNetworkCandidateServiceIdentityFromPayload,
       AdequateLeaderFrozenTargetCorridor,
       AdequateLeaderFrozenTargetCandidateIdentity,
       ExactLeaderFrozenSemanticIdentity,
       AdequateLeaderFrozenCandidatePayload,
       AsyncCandidateServiceIdentity

AdequateLeaderTargetLiveCandidateServiceIdentitySet(
    target, leaderContext, leader, leaderView, subject) ==
  {AsyncCandidateServiceIdentity(candidate):
     candidate \in AsyncCandidateSet,
     rank \in AdequateLeaderTargetSemanticRankCarrier,
     AdequateLeaderFrozenTargetCandidateIdentity(
       candidate, rank, target, leaderContext,
       leader, leaderView, subject),
     CandidateScheduled(candidate)}

THEOREM AdequateLeaderLiveCandidateServiceIdentitiesStayInFrozenCarrier ==
  \A target, leaderContext, leader, leaderView, subject:
    AdequateLeaderFrozenTargetCorridor(
      target, leaderContext, leader, leaderView)
      => AdequateLeaderTargetLiveCandidateServiceIdentitySet(
           target, leaderContext, leader, leaderView, subject)
           \subseteq
         AdequateLeaderFrozenNetworkCandidateServiceIdentityCarrier(
           target, leaderContext, leader, leaderView, subject)
BY AdequateLeaderCandidateServiceIdentityIsInFrozenCarrier, Isa
   DEF AdequateLeaderTargetLiveCandidateServiceIdentitySet

THEOREM AdequateLeaderFrozenNetworkCandidateServiceCarrierIsFinite ==
  \A target, leaderContext, leader, leaderView, subject:
    /\ target \in ValidatorIds
    /\ leaderContext \in ContextRecords
    /\ leader \in ValidatorIds
    /\ leaderView \in Nat
    /\ subject \in Subjects
    => IsFiniteSet(
         AdequateLeaderFrozenNetworkCandidateServiceIdentityCarrier(
           target, leaderContext, leader, leaderView, subject))
BY AdequateLeaderFrozenCandidatePayloadCarrierIsFinite,
   FS_Image, FS_Product, Isa
   DEF AdequateLeaderFrozenNetworkCandidateServiceIdentityCarrier

THEOREM AdequateLeaderCandidateServiceTombstoneTableIsFrozenBounded ==
  \A target, leaderContext, leader, leaderView, subject:
    /\ target \in ValidatorIds
    /\ leaderContext \in ContextRecords
    /\ leader \in ValidatorIds
    /\ leaderView \in Nat
    /\ subject \in Subjects
    /\ AsyncControlServiceStateTypeInvariant
    => Cardinality(
         AdequateLeaderCandidateServiceTombstones(
           target, leaderContext, leader, leaderView, subject))
         <= Cardinality(
              AdequateLeaderFrozenNetworkCandidateServiceIdentityCarrier(
                target, leaderContext, leader, leaderView, subject))
BY AdequateLeaderFrozenNetworkCandidateServiceCarrierIsFinite,
   AsyncCandidateTombstoneSubsetIsBoundedByFrozenOwnerCarrier, Isa
   DEF AdequateLeaderCandidateServiceTombstones

AsyncCandidateServiceTombstonesInIdentityCarrier(carrier) ==
  {record \in AsyncCandidateServiceTombstones:
     record.identity \in carrier}

AsyncCandidateIdentityBudgetBridgeProperty(specification) ==
  /\ (specification
        => []AsyncCandidateServiceTombstoneLifecycleInvariant)
  /\ (specification
        => \A carrier:
             IsFiniteSet(carrier)
               => [](
                    Cardinality(
                      AsyncCandidateServiceTombstonesInIdentityCarrier(
                        carrier))
                      <= Cardinality(carrier)))
  /\ (specification
        => [](\A candidate \in AsyncCandidateSet:
               /\ AsyncCandidateServiceActiveTombstone(candidate)
               /\ [AsyncNext]_AsyncAllVars
               /\ ~AsyncCandidateServiceExitThisStep(candidate)
               => AsyncCandidateServiceActiveTombstone(candidate)'))
  /\ (specification
        => [](\A left, right \in AsyncCandidateSet:
               /\ left.node = right.node
               /\ left.consumerContext = right.consumerContext
               /\ left.height = right.height
               /\ left.view = right.view
               /\ left.subject = right.subject
               /\ left.kind = right.kind
               /\ left.class = right.class
               /\ left.item # NoAsyncItem
               /\ right.item # NoAsyncItem
               /\ left.item.kind = "CertifiedResponse"
               /\ right.item =
                    [left.item EXCEPT !.source = right.item.source]
               /\ AsyncRouteNeutralCandidateEvidence(left.evidence)
                    = AsyncRouteNeutralCandidateEvidence(right.evidence)
               /\ left.bodyIdentity = right.bodyIdentity
               /\ left.manifestIdentity = right.manifestIdentity
               /\ left.commitmentIdentity = right.commitmentIdentity
               => AsyncCandidateServiceIdentity(left)
                    = AsyncCandidateServiceIdentity(right)))
  /\ (specification
        => [](\A identity \in AsyncCandidateAdmissionIdentitySet:
               /\ AsyncCandidateAdmissionIdentityObsolete(identity)
               /\ identity
                    \notin AsyncScheduledCandidateAdmissionIdentities
               /\ gst
               /\ [AsyncNext]_AsyncAllVars
               => /\ AsyncCandidateAdmissionIdentityObsolete(identity)'
                  /\ identity
                       \notin AsyncScheduledCandidateAdmissionIdentities'))
  /\ (specification
        => [](\A identity \in AsyncCandidateAdmissionIdentitySet:
               /\ AsyncCandidateAdmissionIdentityTerminallyCovered(identity)
               /\ identity
                    \notin AsyncScheduledCandidateAdmissionIdentities
               /\ gst
               /\ [AsyncNext]_AsyncAllVars
               => /\ AsyncCandidateAdmissionIdentityTerminallyCovered(
                       identity)'
                  /\ identity
                       \notin AsyncScheduledCandidateAdmissionIdentities'))
  /\ (specification
        => [](\A candidate \in AsyncCandidateSet:
               /\ AsyncLogicalCandidateOwnershipInvariant
               /\ AsyncProgressOwnershipInvariant
               /\ AsyncCandidateServiceLifecycleInvariant
               /\ gst
               /\ AsyncNext
               /\ CandidateScheduled(candidate)
               /\ ~CandidateScheduledAfter(candidate)
               => \/ AsyncCandidateIgnoredWithoutApplicationThisStep(
                        candidate)
                  \/ AsyncCandidateServiceTombstoned(candidate)'
                  \/ AsyncCandidateSameOriginPhysicalOrDurableOwnerAfter(
                        candidate)
                  \/ AsyncCandidateMonotoneSemanticCoverageAfterIn(
                        asyncControlServiceState', candidate)
                  \/ AsyncCandidateTerminalTombstoned(candidate)'))

(***************************************************************************
Terminal retirement is restart-stable only for phases whose durable local
authority survives replay.  The eligibility predicate already excludes the
three signature-completion phases reconstructed from durable intents, and the
tombstone constructor copies the candidate kind verbatim into its phase.  Keep
that bridge explicit so preservation cannot silently admit a restart-scoped
terminal record when either definition evolves.
***************************************************************************)

THEOREM AsyncCandidateTerminalRetirementEligibilityIsRestartSafe ==
  \A candidate:
    AsyncCandidateTerminalRetirementEligibleAfterStep(candidate)
      => candidate.kind \notin AsyncRestartScopedCandidateServiceKinds
BY DEF AsyncCandidateTerminalRetirementEligibleAfterStep

THEOREM AsyncCandidateTerminalTombstoneConstructorPreservesRestartSafety ==
  \A candidate, episodeView, ordinal:
    candidate.kind \notin AsyncRestartScopedCandidateServiceKinds
      => AsyncCandidateServiceTombstone(
           candidate, episodeView, ordinal).phase
             \notin AsyncRestartScopedCandidateServiceKinds
BY DEF AsyncCandidateServiceTombstone

THEOREM AsyncInitEstablishesCandidateServiceTombstoneLifecycle ==
  \A initialContext:
    AsyncInitAt(initialContext)
      => AsyncCandidateServiceTombstoneLifecycleInvariant
BY AsyncInitEstablishesLeaderWireContinuationSharedOrdinalNoCollision,
   Isa
   DEF AsyncInitAt, AsyncBaseInitAt, AsyncTransportInit,
       AsyncRuntimeInit, AsyncIoInit, AsyncDeferredInit,
       AsyncCandidateServiceTombstoneLifecycleInvariant,
       AsyncCandidateServiceLifecycleInvariant,
       AsyncCandidateProducerSemanticHandoffCoverageInvariant,
       AsyncCandidateLifecycleAdmissions,
       AsyncInitialCandidateLifecycleAdmissions,
       AsyncCandidateLifecycleAdmission,
       AsyncCandidateLifecycleStageIdentityInvariant,
       AsyncCandidateScheduledLifecycleStageIdentityInvariant,
       AsyncCandidateRecordedLifecycleStageIdentityInvariant,
       AsyncControlServiceStateTypeInvariant,
       AsyncCandidateServiceTombstones,
       AsyncCandidateServiceRecordsFor,
       AsyncCandidateServiceRecordsForIdentity,
       QueuedCandidates, DeferredCandidates,
       CausalCandidates, TrackedWorkCandidates,
       SequenceSet

THEOREM AsyncNextPreservesCandidateServiceTombstoneLifecycle ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncProgressOwnershipInvariant
  /\ AsyncCandidateServiceTombstoneLifecycleInvariant
  /\ AsyncNext
  => AsyncCandidateServiceTombstoneLifecycleInvariant'
BY AsyncNextPreservesControlServiceStateTypeInvariant,
   AsyncNextPreservesLeaderWireContinuationSharedOrdinalNoCollision,
   AsyncControlServiceTransitionPreservesSemanticHandoffCoverage,
   AsyncCandidateServicesThisStepIsSingleton,
   AsyncCandidateTerminalRetirementsThisStepIsSingleton,
   AsyncCandidateTerminalRetirementEligibilityIsRestartSafe,
   AsyncCandidateTerminalTombstoneConstructorPreservesRestartSafety,
   AsyncCandidateSuccessfulServiceInstallsTombstone,
   AsyncCandidateDiscardInstallsTerminalTombstone,
   AsyncCandidateCausalAdmissionTransfersSameOwner,
   AsyncCandidateIoCompletionTransfersSameOwner,
   AsyncCandidateProducerCompletionTransfersSameOwner,
   AsyncCandidateBusyDeferralTransfersSameOwner,
   AsyncCandidateDeferredHandoffRetainsSameOwner,
   AsyncCandidateDiscardIsNotSemanticService,
   AsyncCandidateServiceTombstoneCoalescesFreshCandidate,
   AsyncCandidateServiceTombstoneRejectsTransportReadmission,
   AsyncCandidateSameHeightRestartPreservesServicedIdentity,
   IsaT(600)
   DEF AsyncCandidateServiceTombstoneLifecycleInvariant,
       AsyncCandidateServiceLifecycleInvariant,
       AsyncCandidateLifecycleStageIdentityInvariant,
       AsyncCandidateScheduledLifecycleStageIdentityInvariant,
       AsyncCandidateRecordedLifecycleStageIdentityInvariant,
       AsyncStrongTypeInvariant,
       AsyncProgressOwnershipInvariant,
       AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       RunNode, RunHistoricalRecoveryNode, RunHistoricalServer,
       RunNodeWork,
       ResolveRunNodeCandidateProducerContinuation,
       ReplayRunNodeCandidateProducerContinuation,
       AsyncCandidateProducerContinuationExactLocalReplayStep,
       AsyncCandidateProducerContinuationReplayTargetOnlyTurn,
       AsyncCandidateProducerContinuationExactRuntimeReplayStep,
       AsyncCandidateProducerContinuationExactReplayIdentity,
       AsyncCandidateProducerContinuationSelectedLocalCandidate,
       AsyncCandidateProducerContinuationSelectedRuntimeCandidate,
       AsyncCandidateProducerContinuationSelectedReplayRecord,
       AsyncCandidateProducerContinuationSelectedResolutionRecord,
       AsyncCandidateProducerContinuationResolutionRequired,
       AsyncCandidateProducerContinuationResolutionReady,
       AsyncCandidateProducerContinuationResolutionRecordsForNode,
       AsyncSchedulerExceptCausalControlAndNodeService,
       AsyncSchedulerExceptCausalControlCommandRunnerAndNodeService,
       AsyncSchedulerExceptCausalControlRunnerAndNodeService,
       LocalAdmissionStep, IngressDrainStep,
       SerializedRuntimeStep, RuntimeStep,
       DrainFairIngressSelected, AdmitCausalHead,
       AdmitProducerCompletion, ServiceIoWorkerWork,
       FifoRuntimeStep, DeferredDrainStep,
       AppendCausalSuccessors, FreshCommandSuccessors,
       AsyncCandidateTerminalRetirementsThisStep,
       AsyncCandidateTerminalDiscardsThisStep,
       AsyncCandidateTerminallyDiscardedThisStep,
       AsyncCandidateServiceStateAfterTerminalRetirement,
       AsyncCandidateTerminalRetirementEligibleAfterStep,
       AsyncCandidateServiceTombstone,
       FreshCandidateSequence, CandidateAdmissionCoalesced,
       AdmitIngressPacket, AdmitHiddenPacket,
       CoalesceHiddenPacket, DropPolicyRejectedHiddenPacket,
       DriveResponsiveReplayHead, FinishResponsiveReplay,
       PreGstResponsiveReplay, ResetNodeSchedulerForRestart,
       FreshRestartCandidateSequence,
       CandidateScheduled, CandidateScheduledAfter

THEOREM AsyncSpecAlwaysCandidateServiceTombstoneLifecycle ==
  \A initialContext:
    AsyncSpecAt(initialContext)
      => []AsyncCandidateServiceTombstoneLifecycleInvariant
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncSpecAt(initialContext)
         PROVE []AsyncCandidateServiceTombstoneLifecycleInvariant
    <2>1. AsyncInitAt(initialContext)
             => AsyncCandidateServiceTombstoneLifecycleInvariant
      BY AsyncInitEstablishesCandidateServiceTombstoneLifecycle
    <2>2. []AsyncStrongTypeInvariant
      BY <1>1, AsyncSpecAlwaysStrongTypeInvariant
    <2>3. []AsyncProgressOwnershipInvariant
      BY <1>1, AsyncSpecAlwaysProgressOwnershipInvariant
    <2>4. /\ AsyncStrongTypeInvariant
           /\ AsyncProgressOwnershipInvariant
           /\ AsyncCandidateServiceTombstoneLifecycleInvariant
           /\ [AsyncNext]_AsyncAllVars
          => AsyncCandidateServiceTombstoneLifecycleInvariant'
      BY AsyncNextPreservesCandidateServiceTombstoneLifecycle, Isa
         DEF AsyncAllVars
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4, PTL
         DEF AsyncSpecAt
  <1> QED BY <1>1

THEOREM AsyncLiveProvidesAdequateLeaderTargetOffSubjectControlNoReentry ==
  \A initialContext:
    AdequateLeaderTargetOffSubjectControlNoReentryProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysProgressOwnershipInvariant,
   AsyncSpecAlwaysCandidateServiceTombstoneLifecycle,
   AsyncLiveSpecProjectsAsyncSpec,
   AdequateLeaderOffSubjectControlRetirementMemoryIsStepInvariant,
   GstAsyncStepIsMonotone,
   PTL
   DEF AdequateLeaderTargetOffSubjectControlNoReentryProperty,
       AdequateLeaderTargetOffSubjectControlOccurrenceIdentity,
       AdequateLeaderTargetOffSubjectControlRetirementClosed,
       AdequateLeaderTargetOffSubjectControlRetirementMemory,
       AsyncAllVars

(***************************************************************************
An authenticated exact occurrence which is later discarded cannot disappear
into a reusable old stage.  If Decision did not make its lifecycle obsolete,
the terminal-discard transition installs the route-neutral tombstone; the
ordinary candidate admission constructor and the transport admission gate
then both exclude recreation.  Signature-completion phases are deliberately
outside this theorem because replay reconstructs them from durable intents.
***************************************************************************)

THEOREM AuthenticatedExactLeaderTerminalDiscardInstallsClosedTombstone ==
  \A candidate \in AsyncCandidateSet,
     rank \in (1..5) \X Nat:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateServiceTombstoneLifecycleInvariant
    /\ ExactLeaderCandidateRank(candidate, rank)
    /\ AuthenticatedLeaderDiscardProvenance(candidate)
    /\ candidate.kind \notin AsyncRestartScopedCandidateServiceKinds
    /\ SameConsumerLeaderDiscard(candidate)
    /\ AsyncNext
    => /\ ExactLeaderDiscardProvenanceAt(candidate, rank)'
       /\ ~CandidateScheduled(candidate)'
       /\ \/ NodeHasDecision(candidate.node)'
          \/ /\ AsyncCandidateTerminalTombstoned(candidate)'
             /\ CandidateAdmissionCoalesced(candidate)'
             /\ FreshCandidateSequence(candidate)' = <<>>
BY ExactLeaderDiscardProvenanceSurvivesSameConsumerDiscard,
   AsyncCandidateDiscardInstallsTerminalTombstone,
   AsyncCandidateTerminalTombstoneCoalescesFreshCandidate,
   AsyncCandidateTerminalRetirementEligibilityIsRestartSafe,
   AsyncCandidateTerminalTombstoneConstructorPreservesRestartSafety,
   IsaT(600)
   DEF ExactLeaderDiscardProvenanceAt,
       AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       SameConsumerLeaderDiscard,
       AsyncCandidateTerminallyDiscardedThisStep,
       AsyncCandidateTerminalRetirementEligibleAfterStep,
       AsyncCandidateTerminalTombstoned,
       CandidateScheduled, CandidateScheduledAfter,
       CandidateScheduledIn,
       FifoRuntimeStep, DeferredDrainStep,
       RemoveNextNodeCommand, RemoveNextDeferredCommand,
       DiscardCommand, AsyncNext

THEOREM AsyncSpecAlwaysExactLeaderSchedulerOriginInductionSafety ==
  \A initialContext:
    AsyncSpecAt(initialContext)
      => []ExactLeaderSchedulerOriginInductionSafetyContext
BY AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysProgressOwnershipInvariant,
   DecisionFrontierUniquenessInvariantFromAsyncSpec,
   DecisionTimeoutFrontierInvariantFromAsyncSpec,
   ResponsiveRecoveryValidationClearedInvariantObligation,
   FinalProgressWitnessClosureInvariantObligation,
   ReplayTailCommitReadyInvariantObligation,
   AsyncSpecAlwaysCandidateServiceTombstoneLifecycle, PTL
   DEF ExactLeaderSchedulerOriginInductionSafetyContext

THEOREM AsyncSpecAlwaysExactLeaderSchedulerOriginReadiness ==
  \A initialContext:
    ExactLeaderSchedulerOriginReadinessProperty(
      AsyncSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE ExactLeaderSchedulerOriginReadinessProperty(
                 AsyncSpecAt(initialContext))
    <2>1. AsyncInitAt(initialContext)
             => ExactLeaderSchedulerOriginReadinessInvariant
      BY AsyncInitEstablishesExactLeaderSchedulerOriginReadiness
    <2>2. AsyncSpecAt(initialContext)
             => []ExactLeaderSchedulerOriginInductionSafetyContext
      BY AsyncSpecAlwaysExactLeaderSchedulerOriginInductionSafety
    <2>3. /\ ExactLeaderSchedulerOriginInductionSafetyContext
           /\ ExactLeaderSchedulerOriginReadinessInvariant
           /\ [AsyncNext]_AsyncAllVars
          => ExactLeaderSchedulerOriginReadinessInvariant'
      BY AsyncBracketNextPreservesExactLeaderSchedulerOriginReadiness
         DEF ExactLeaderSchedulerOriginInductionContext
    <2> QED BY <2>1, <2>2, <2>3, PTL
         DEF ExactLeaderSchedulerOriginReadinessProperty, AsyncSpecAt
  <1> QED BY <1>1

THEOREM AsyncLiveExactLeaderSchedulerOriginReadiness ==
  \A initialContext:
    ExactLeaderSchedulerOriginReadinessProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncSpecAlwaysExactLeaderSchedulerOriginReadiness,
   AsyncLiveSpecProjectsAsyncSpec, PTL
   DEF ExactLeaderSchedulerOriginReadinessProperty

THEOREM AsyncLiveProvidesAdequateLeaderExactResidualKernel ==
  \A initialContext:
    AdequateLeaderExactResidualKernelProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncLiveExactLeaderSchedulerOriginReadiness,
   AsyncLiveProvidesAdequateLeaderOpenPhysicalResidualConvergence
   DEF AdequateLeaderExactResidualKernelProperty

THEOREM AsyncLiveResponsiveExactLeaderSchedulerSourcesAreUp ==
  \A initialContext:
    AsyncLiveSpecAt(initialContext)
      => [](\A candidate \in AsyncCandidateSet,
                rank \in (1..5) \X Nat:
             /\ gst
             /\ ExactLeaderCandidateRank(candidate, rank)
             => candidate.node \in up)
BY AsyncSpecAlwaysStrongTypeInvariant,
   AsyncLiveSpecProjectsAsyncSpec,
   GstResponsiveNodesAreUp, PTL
   DEF AsyncStrongTypeInvariant,
       ExactLeaderCandidateRank,
       ResponsiveProtectedCandidateOwned,
       ProtectedCandidateOwned,
       ProtectedServiceCandidate,
       AsyncCurrentResponsiveVoters

THEOREM AsyncSpecProvidesHistoricalDiscoveryCandidateIdentityBudgetBridge ==
  \A initialContext:
    AsyncCandidateIdentityBudgetBridgeProperty(
      AsyncSpecAt(initialContext))
BY AsyncSpecAlwaysCandidateServiceTombstoneLifecycle,
   AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysProgressOwnershipInvariant,
   AsyncCandidateTombstoneSubsetIsBoundedByFrozenOwnerCarrier,
   AsyncCandidateServicedIdentityCannotReactivate,
   AsyncCandidateAdmissionIdentityObsolescenceIsMonotoneAtGst,
   AsyncCandidateObsoleteAdmissionIdentityCannotReappearAtGst,
   AsyncCandidateTerminalIdentityCannotReactivateAtGst,
   AsyncCandidateScheduledIdentityDepartureRetiresLifecycleAtGst,
   AsyncCandidateServiceRouteNeutralResponseRetryIsStable,
   Isa, PTL
   DEF AsyncCandidateIdentityBudgetBridgeProperty,
       AsyncCandidateServiceTombstonesInIdentityCarrier,
       AsyncStrongTypeInvariant,
       AsyncAllVars

AdequateLeaderCandidateFrozenIdentityBudgetBridgeProperty(specification) ==
  /\ AsyncCandidateIdentityBudgetBridgeProperty(specification)
  /\ (specification
        => \A target \in ValidatorIds,
              leaderContext \in ContextRecords,
              leader \in ValidatorIds,
              leaderView \in Views,
              subject \in Subjects:
             /\ IsFiniteSet(
                  AdequateLeaderFrozenNetworkCandidateServiceIdentityCarrier(
                    target, leaderContext, leader, leaderView, subject))
             /\ [](AdequateLeaderFrozenTargetCorridor(
                       target, leaderContext, leader, leaderView)
                    => AdequateLeaderTargetLiveCandidateServiceIdentitySet(
                         target, leaderContext, leader, leaderView, subject)
                         \subseteq
                       AdequateLeaderFrozenNetworkCandidateServiceIdentityCarrier(
                         target, leaderContext, leader, leaderView, subject))
             /\ [](Cardinality(
                       AdequateLeaderCandidateServiceTombstones(
                         target, leaderContext, leader, leaderView, subject))
                     <= Cardinality(
                          AdequateLeaderFrozenNetworkCandidateServiceIdentityCarrier(
                            target, leaderContext, leader,
                            leaderView, subject))))

THEOREM AsyncSpecProvidesAdequateLeaderCandidateFrozenIdentityBudgetBridge ==
  \A initialContext:
    AdequateLeaderCandidateFrozenIdentityBudgetBridgeProperty(
      AsyncSpecAt(initialContext))
BY AsyncSpecProvidesHistoricalDiscoveryCandidateIdentityBudgetBridge,
   AsyncSpecAlwaysStrongTypeInvariant,
   AdequateLeaderFrozenNetworkCandidateServiceCarrierIsFinite,
   AdequateLeaderLiveCandidateServiceIdentitiesStayInFrozenCarrier,
   AdequateLeaderCandidateServiceTombstoneTableIsFrozenBounded,
   PTL
   DEF AdequateLeaderCandidateFrozenIdentityBudgetBridgeProperty,
       AsyncStrongTypeInvariant

AdequateLeaderTargetServicedCandidateOwnerIdentitySet(
    target, leaderContext, leader, leaderView, subject) ==
  {AdequateLeaderFrozenCandidateOwnerIdentity(
     candidate, rank, target, leaderContext,
     leader, leaderView, subject):
     candidate \in AsyncCandidateSet,
     rank \in AdequateLeaderTargetSemanticRankCarrier,
     AdequateLeaderFrozenTargetCandidateIdentity(
       candidate, rank, target, leaderContext,
       leader, leaderView, subject),
     AsyncCandidateServiceTombstoned(candidate)}

(***************************************************************************
Producer-continuation retirement memory.

Across a pre-GST restart, a Terminal producer-continuation record is stable
exactly when its retirement is durable, it has the matching retained
candidate-service tombstone, or the exact lifecycle reservation is no longer
covered.  That three-way predicate remains available below for reset/replay
consumers.

Inside the frozen post-GST corridor, restart is disabled.  The selected
owner's exact Terminal record is therefore sufficient retirement memory.  If
that record is reclaimed to admit a strictly newer view at the same bounded
lifecycle-slot/stage address, the newer record is the monotone high-water mark
which prevents resurrection of the old stage.  Neither branch is Decision,
occurrence-rank descent, producer progress, a new state field, or an
environmental assumption.
***************************************************************************)
AdequateLeaderCandidateProducerContinuationRestartStableRetirementMemory(
    candidate) ==
  \/ \E record \in AsyncCandidateProducerContinuations:
       /\ record.identity = AsyncCandidateServiceIdentity(candidate)
       /\ AsyncCandidateProducerContinuationRestartStableTerminalIn(
            asyncControlServiceState, record)
  \/ \E record \in AsyncCandidateProducerContinuations:
       /\ record.node = candidate.node
       /\ record.context = candidate.consumerContext
       /\ record.height = candidate.height
       /\ record.address.stage
            = AsyncCandidateServiceStageForKind(candidate.kind)
       /\ record.view > candidate.view
       /\ AsyncCandidateProducerContinuationRestartStableTerminalIn(
            asyncControlServiceState, record)

AdequateLeaderCandidateProducerContinuationRetirementMemory(candidate) ==
  \/ AsyncCandidateProducerContinuationTerminalForIdentity(
       AsyncCandidateServiceIdentity(candidate))
  \/ \E record \in AsyncCandidateProducerContinuations:
       /\ record.node = candidate.node
       /\ record.context = candidate.consumerContext
       /\ record.height = candidate.height
       /\ record.address.stage
            = AsyncCandidateServiceStageForKind(candidate.kind)
       /\ record.view > candidate.view

THEOREM AdequateLeaderRestartStableContinuationRetirementRefinesPostGstMemory ==
  \A candidate \in AsyncCandidateSet:
    AdequateLeaderCandidateProducerContinuationRestartStableRetirementMemory(
      candidate)
      => AdequateLeaderCandidateProducerContinuationRetirementMemory(candidate)
BY Isa
   DEF AdequateLeaderCandidateProducerContinuationRestartStableRetirementMemory,
       AdequateLeaderCandidateProducerContinuationRetirementMemory,
       AsyncCandidateProducerContinuationRestartStableTerminalIn,
       AsyncCandidateProducerContinuationTerminalForIdentity,
       AsyncCandidateProducerContinuationTerminalForIdentityIn

AdequateLeaderTargetProducerContinuationRetiredOwnerIdentitySet(
    target, leaderContext, leader, leaderView, subject) ==
  {AdequateLeaderFrozenCandidateOwnerIdentity(
     candidate, rank, target, leaderContext,
     leader, leaderView, subject):
     candidate \in AsyncCandidateSet,
     rank \in AdequateLeaderTargetSemanticRankCarrier,
     AdequateLeaderFrozenTargetCandidateIdentity(
       candidate, rank, target, leaderContext,
       leader, leaderView, subject),
     AdequateLeaderCandidateProducerContinuationRetirementMemory(candidate)}

AdequateLeaderTargetDecisionRetiredCandidateOwnerIdentitySet(
    target, leaderContext, leader, leaderView, subject) ==
  {identity \in
     AdequateLeaderFrozenCandidateOwnerUniverse(
       target, leaderContext, leader, leaderView, subject):
     NodeHasDecision(identity.owner)}

THEOREM AdequateLeaderProducerContinuationRetiredOwnersStayInFrozenUniverse ==
  \A target, leaderContext, leader, leaderView, subject:
    AdequateLeaderTargetProducerContinuationRetiredOwnerIdentitySet(
      target, leaderContext, leader, leaderView, subject)
      \subseteq
    AdequateLeaderFrozenCandidateOwnerUniverse(
      target, leaderContext, leader, leaderView, subject)
BY AdequateLeaderFrozenTargetCandidatePayloadIsInStaticCarrier,
   IsaT(300)
   DEF AdequateLeaderTargetProducerContinuationRetiredOwnerIdentitySet,
       AdequateLeaderFrozenCandidateOwnerUniverse,
       AdequateLeaderFrozenCandidateOwnerIdentity,
       AdequateLeaderFrozenCandidateOwnerIdentityFromPayload

THEOREM AdequateLeaderProducerContinuationRetiredOwnerSetIsFinite ==
  \A target, leaderContext, leader, leaderView, subject:
    /\ target \in ValidatorIds
    /\ leaderContext \in ContextRecords
    /\ leader \in ValidatorIds
    /\ leaderView \in Nat
    /\ subject \in Subjects
    => IsFiniteSet(
         AdequateLeaderTargetProducerContinuationRetiredOwnerIdentitySet(
           target, leaderContext, leader, leaderView, subject))
BY AdequateLeaderProducerContinuationRetiredOwnersStayInFrozenUniverse,
   AdequateLeaderFrozenOwnerUniverseIsFinite, FS_Subset, Isa
   DEF AdequateLeaderFrozenOwnerUniverse

THEOREM AdequateLeaderProducerContinuationRetiredOwnersAreNotLive ==
  \A target, leaderContext, leader, leaderView, subject:
    AsyncCandidateServiceLifecycleInvariant
      => AdequateLeaderTargetProducerContinuationRetiredOwnerIdentitySet(
           target, leaderContext, leader, leaderView, subject)
           \cap
         AdequateLeaderTargetLiveCandidateOwnerIdentitySet(
           target, leaderContext, leader, leaderView, subject)
           = {}
BY AdequateLeaderOwnerIdentityDeterminesNetworkServiceIdentity,
   IsaT(900)
   DEF AdequateLeaderTargetProducerContinuationRetiredOwnerIdentitySet,
       AdequateLeaderCandidateProducerContinuationRetirementMemory,
       AdequateLeaderTargetLiveCandidateOwnerIdentitySet,
       AdequateLeaderTargetRankOwnerIdentitySet,
       AdequateLeaderTargetRankOwnerSet,
       AdequateLeaderTargetCandidateIdentity,
       AdequateLeaderFrozenTargetCandidateIdentity,
       AsyncCandidateServiceLifecycleInvariant,
       AsyncCandidateProducerContinuationScheduledExclusionInvariant,
       AsyncCandidateProducerContinuationBlocks,
       AsyncCandidateProducerContinuationTerminalForIdentity,
       AsyncCandidateProducerContinuationTerminalForIdentityIn,
       AsyncCandidateProducerContinuationRecordsForIdentityIn,
       CandidateScheduled, CandidateScheduledIn,
       QueuedCandidates, DeferredCandidates,
       CausalCandidates, TrackedWorkCandidates

THEOREM AdequateLeaderCandidateProducerContinuationRetirementMemoryPersists ==
  \A target, leaderContext, leader, leaderView,
     subject, rank, candidate:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ gst
    /\ AdequateLeaderFrozenTargetCorridor(
         target, leaderContext, leader, leaderView)
    /\ AdequateLeaderFrozenTargetCandidateIdentity(
         candidate, rank, target, leaderContext,
         leader, leaderView, subject)
    /\ AdequateLeaderCandidateProducerContinuationRetirementMemory(candidate)
    /\ [AsyncNext]_AsyncAllVars
    /\ AdequateLeaderFrozenTargetCorridor(
         target, leaderContext, leader, leaderView)'
    => \/ NodeHasDecision(candidate.node)'
       \/ AdequateLeaderCandidateProducerContinuationRetirementMemory(candidate)'
BY AsyncCandidateProducerContinuationTerminalRecordIsFixed,
   AsyncCandidateProducerContinuationReclamationPreservesIdentity,
   AsyncCandidateProducerContinuationDecisionReclamationClearsNode,
   AsyncCandidateProducerContinuationSameHeightRestartPreserved,
   AsyncCandidateProducerContinuationReplacementRetiresOnlyTerminal,
   AsyncCandidateProducerContinuationHighWatermarkBlocksOldStage,
   IsaT(2400)
   DEF AdequateLeaderCandidateProducerContinuationRetirementMemory,
       AsyncCandidateProducerContinuationTerminalForIdentity,
       AsyncCandidateProducerContinuationTerminalForIdentityIn,
       AsyncCandidateProducerContinuationRecordsForIdentityIn,
       AsyncCandidateProducerContinuationStateAfterDeparture,
       AsyncCandidateProducerContinuationAddressCanAdvanceIn,
       AsyncCandidateProducerContinuationRecordsForAddressIn,
       AsyncCandidateProducerContinuationRecordAfterStep,
       AsyncCandidateServiceStateAfterReclamation,
       AsyncControlServiceStateAfterReset,
       AsyncControlServiceSlotTransition,
       AsyncCandidateProducerContinuationBlocks,
       AdequateLeaderFrozenTargetCorridor,
       AsyncNext, AsyncAllVars

THEOREM AdequateLeaderProducerContinuationRetiredOwnerSetIsMonotoneAtGst ==
  \A target, leaderContext, leader, leaderView, subject:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ gst
    /\ AdequateLeaderFrozenTargetCorridor(
         target, leaderContext, leader, leaderView)
    /\ [AsyncNext]_AsyncAllVars
    /\ AdequateLeaderFrozenTargetCorridor(
         target, leaderContext, leader, leaderView)'
    => AdequateLeaderTargetProducerContinuationRetiredOwnerIdentitySet(
         target, leaderContext, leader, leaderView, subject)
         \subseteq
       AdequateLeaderTargetProducerContinuationRetiredOwnerIdentitySet(
         target, leaderContext, leader, leaderView, subject)'
         \cup
       AdequateLeaderTargetDecisionRetiredCandidateOwnerIdentitySet(
         target, leaderContext, leader, leaderView, subject)'
BY AdequateLeaderCandidateProducerContinuationRetirementMemoryPersists,
   AdequateLeaderFrozenOwnerUniverseIsPrimeInvariant,
   IsaT(900)
   DEF AdequateLeaderTargetProducerContinuationRetiredOwnerIdentitySet,
       AdequateLeaderTargetDecisionRetiredCandidateOwnerIdentitySet,
       AdequateLeaderFrozenOwnerUniverse,
       AdequateLeaderFrozenCandidateOwnerUniverse,
       AdequateLeaderFrozenTargetCandidateIdentity,
       AdequateLeaderFrozenCandidateOwnerIdentity,
       AdequateLeaderFrozenCandidateOwnerIdentityFromPayload,
       AsyncAllVars

(***************************************************************************
BodyAvailable preflight retirement.

The serviced-candidate table is not the only durable reason an internal body
callback cannot return.  Once the reducer's exact body has left Missing, the
production preflight consumes no new admission ordinal; the Async model now
matches that boundary in FreshCandidateSequence.  The set below records the
same immutable frozen owner identity only after its service identity has left
all scheduler carriers.  It therefore closes the historical
FetchBody -> FetchCertifiedBody -> FetchBody lasso without pretending that
the response itself serviced the old FetchBody occurrence.
***************************************************************************)
AdequateLeaderTargetInternalBodyAvailableRetiredOwnerIdentitySet(
    target, leaderContext, leader, leaderView, subject) ==
  {AdequateLeaderFrozenCandidateOwnerIdentity(
     candidate, rank, target, leaderContext,
     leader, leaderView, subject):
     candidate \in AsyncCandidateSet,
     rank \in AdequateLeaderTargetSemanticRankCarrier,
     AdequateLeaderFrozenTargetCandidateIdentity(
       candidate, rank, target, leaderContext,
       leader, leaderView, subject),
     AsyncCandidateInternalBodyAvailableStageRetired(candidate),
     AsyncCandidateServiceIdentity(candidate)
       \notin AsyncScheduledCandidateServiceIdentities}

THEOREM AdequateLeaderInternalBodyAvailableRetiredOwnersAreNotLive ==
  \A target, leaderContext, leader, leaderView, subject:
    AdequateLeaderTargetInternalBodyAvailableRetiredOwnerIdentitySet(
      target, leaderContext, leader, leaderView, subject)
      \cap
    AdequateLeaderTargetLiveCandidateOwnerIdentitySet(
      target, leaderContext, leader, leaderView, subject)
      = {}
BY AdequateLeaderOwnerIdentityDeterminesNetworkServiceIdentity,
   IsaT(600)
   DEF AdequateLeaderTargetInternalBodyAvailableRetiredOwnerIdentitySet,
       AdequateLeaderTargetLiveCandidateOwnerIdentitySet,
       AdequateLeaderTargetRankOwnerIdentitySet,
       AdequateLeaderTargetRankOwnerSet,
       AdequateLeaderTargetCandidateIdentity,
       AdequateLeaderFrozenTargetCandidateIdentity,
       AsyncScheduledCandidateServiceIdentities,
       CandidateScheduled, QueuedCandidates, DeferredCandidates,
       CausalCandidates, TrackedWorkCandidates

THEOREM AdequateLeaderInternalBodyAvailableRetiredOwnerSetIsMonotoneAtGst ==
  \A target, leaderContext, leader, leaderView, subject:
    /\ gst
    /\ [AsyncNext]_AsyncAllVars
    => AdequateLeaderTargetInternalBodyAvailableRetiredOwnerIdentitySet(
         target, leaderContext, leader, leaderView, subject)
         \subseteq
       AdequateLeaderTargetInternalBodyAvailableRetiredOwnerIdentitySet(
         target, leaderContext, leader, leaderView, subject)'
BY AsyncCandidateInternalBodyAvailableServiceIdentityCannotReactivateAtGst,
   AdequateLeaderFrozenOwnerUniverseIsPrimeInvariant,
   IsaT(600)
   DEF AdequateLeaderTargetInternalBodyAvailableRetiredOwnerIdentitySet,
       AdequateLeaderFrozenOwnerUniverse,
       AdequateLeaderFrozenCandidateOwnerUniverse,
       AdequateLeaderFrozenTargetCandidateIdentity,
       ExactLeaderFrozenSemanticIdentity,
       AdequateLeaderFrozenTargetCandidateRole,
       AsyncAllVars

AdequateLeaderTargetInternalBodyAvailableNoReentryProperty(specification) ==
  specification
    => [](\A target \in ValidatorIds,
             leaderContext \in ContextRecords,
             leader \in ValidatorIds,
             leaderView \in Views,
             subject \in Subjects,
             owner \in
               AdequateLeaderFrozenCandidateOwnerUniverse(
                 target, leaderContext, leader, leaderView, subject):
            /\ gst
            /\ owner \in
                 AdequateLeaderTargetInternalBodyAvailableRetiredOwnerIdentitySet(
                   target, leaderContext, leader, leaderView, subject)
              => [](/\ owner \in
                           AdequateLeaderTargetInternalBodyAvailableRetiredOwnerIdentitySet(
                             target, leaderContext, leader,
                             leaderView, subject)
                     /\ owner \notin
                           AdequateLeaderTargetLiveCandidateOwnerIdentitySet(
                             target, leaderContext, leader,
                             leaderView, subject)))

THEOREM AsyncSpecProvidesAdequateLeaderTargetInternalBodyAvailableNoReentry ==
  \A initialContext:
    AdequateLeaderTargetInternalBodyAvailableNoReentryProperty(
      AsyncSpecAt(initialContext))
BY AdequateLeaderInternalBodyAvailableRetiredOwnersAreNotLive,
   AdequateLeaderInternalBodyAvailableRetiredOwnerSetIsMonotoneAtGst,
   GstAsyncStepIsMonotone,
   PTL
   DEF AdequateLeaderTargetInternalBodyAvailableNoReentryProperty,
       AsyncAllVars

THEOREM AsyncLiveProvidesAdequateLeaderTargetInternalBodyAvailableNoReentry ==
  \A initialContext:
    AdequateLeaderTargetInternalBodyAvailableNoReentryProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncSpecProvidesAdequateLeaderTargetInternalBodyAvailableNoReentry,
   AsyncLiveSpecProjectsAsyncSpec, PTL
   DEF AdequateLeaderTargetInternalBodyAvailableNoReentryProperty

THEOREM AdequateLeaderLiveAndServicedCandidateIdentitiesAreDisjoint ==
  \A target, leaderContext, leader, leaderView, subject:
    AsyncCandidateServiceTombstoneLifecycleInvariant
      => AdequateLeaderTargetLiveCandidateOwnerIdentitySet(
           target, leaderContext, leader, leaderView, subject)
           \cap
         AdequateLeaderTargetServicedCandidateOwnerIdentitySet(
           target, leaderContext, leader, leaderView, subject)
           = {}
BY AdequateLeaderOwnerIdentityDeterminesNetworkServiceIdentity,
   IsaT(600)
   DEF AsyncCandidateServiceTombstoneLifecycleInvariant,
       AdequateLeaderTargetLiveCandidateOwnerIdentitySet,
       AdequateLeaderTargetRankOwnerIdentitySet,
       AdequateLeaderTargetRankOwnerSet,
       AdequateLeaderTargetCandidateIdentity,
       AdequateLeaderFrozenTargetCandidateIdentity,
       AdequateLeaderTargetServicedCandidateOwnerIdentitySet,
       AsyncCandidateServiceTombstoned,
       AsyncCandidateServiceRecordsFor,
       AsyncCandidateServiceRecordsForIdentity,
       CandidateScheduled, QueuedCandidates, DeferredCandidates,
       CausalCandidates, TrackedWorkCandidates

THEOREM AdequateLeaderServicedCandidateIdentityHasServiceWitness ==
  \A target, leaderContext, leader, leaderView, subject, identity:
    /\ AsyncCandidateServiceTombstoneLifecycleInvariant
    /\ identity \in
         AdequateLeaderTargetServicedCandidateOwnerIdentitySet(
           target, leaderContext, leader, leaderView, subject)
    => \E candidate \in AsyncCandidateSet,
          rank \in AdequateLeaderTargetSemanticRankCarrier:
         /\ AdequateLeaderFrozenTargetCandidateIdentity(
              candidate, rank, target, leaderContext,
              leader, leaderView, subject)
         /\ identity =
              AdequateLeaderFrozenCandidateOwnerIdentity(
                candidate, rank, target, leaderContext,
                leader, leaderView, subject)
         /\ AsyncCandidateServiceTombstoned(candidate)
BY IsaT(300)
   DEF AdequateLeaderTargetServicedCandidateOwnerIdentitySet,
       AsyncCandidateServiceTombstoneLifecycleInvariant,
       AsyncCandidateServiceLifecycleInvariant,
       AsyncControlServiceStateTypeInvariant,
       AsyncCandidateServiceOwnerPartitionInvariantIn,
       AsyncCandidateServiceTombstoned,
       AsyncCandidateServiceCoalesced

AdequateLeaderServicedCandidateMemory(
    target, leaderContext, leader, leaderView, subject, identity) ==
  \/ identity \in
       AdequateLeaderTargetServicedCandidateOwnerIdentitySet(
         target, leaderContext, leader, leaderView, subject)
  \/ context # leaderContext
  \/ nodeView[target] > leaderView
  \/ nodeView[leader] > leaderView
  \/ NodeHasDecision(identity.owner)

THEOREM AdequateLeaderCandidateSuccessfulServiceRetirementInstallsServicedMemory ==
  \A target, leaderContext, leader, leaderView,
     subject, occurrenceRank, identity:
    /\ AsyncCandidateServiceTombstoneLifecycleInvariant
    /\ AdequateLeaderTargetCandidateSuccessfulServiceRetirementAction(
         target, leaderContext, leader, leaderView,
         subject, occurrenceRank, identity)
    => \E candidate \in AsyncCandidateSet,
          rank \in AdequateLeaderTargetSemanticRankCarrier:
         /\ AdequateLeaderTargetCandidateOwnerIdentityWitness(
              target, leaderContext, leader, leaderView,
              subject, identity, candidate, rank)
         /\ AsyncCandidateServiceTombstoned(candidate)'
         /\ ~CandidateScheduled(candidate)'
         /\ AdequateLeaderServicedCandidateMemory(
              target, leaderContext, leader, leaderView,
              subject, identity)'
BY AsyncCandidateSuccessfulServiceInstallsTransientMarker,
   AsyncCandidateSuccessfulServiceInstallsTombstone,
   AdequateLeaderOwnerIdentityDeterminesNetworkServiceIdentity,
   IsaT(300)
   DEF AdequateLeaderTargetCandidateSuccessfulServiceRetirementAction,
       AdequateLeaderServicedCandidateMemory,
       AdequateLeaderTargetServicedCandidateOwnerIdentitySet

THEOREM AdequateLeaderCandidateTerminalRetirementInstallsServicedMemory ==
  \A target, leaderContext, leader, leaderView,
     subject, occurrenceRank, identity:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateServiceTombstoneLifecycleInvariant
    /\ AdequateLeaderTargetCandidateTerminalDiscardRetirementAction(
         target, leaderContext, leader, leaderView,
         subject, occurrenceRank, identity)
    => AdequateLeaderServicedCandidateMemory(
         target, leaderContext, leader, leaderView, subject, identity)'
BY AsyncCandidateDiscardInstallsTerminalTombstone, IsaT(600)
   DEF AdequateLeaderTargetCandidateTerminalDiscardRetirementAction,
       AdequateLeaderTargetCandidateOwnerIdentityRetirementAction,
       AdequateLeaderTargetOwnerIdentityRetirementAction,
       AdequateLeaderTargetCandidateOwnerIdentityWitness,
       AdequateLeaderTargetServicedCandidateOwnerIdentitySet,
       AdequateLeaderServicedCandidateMemory,
       AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncCandidateServiceTombstoned,
       AsyncCandidateServiceCoalesced

AdequateLeaderServicedCandidateClosure(
    target, leaderContext, leader, leaderView,
    subject, occurrenceRank, identity) ==
  \/ AdequateLeaderTargetStrictOccurrenceDescentGoal(
       target, leaderContext, leader,
       leaderView, subject, occurrenceRank)
  \/ identity \notin
       AdequateLeaderTargetLiveCandidateOwnerIdentitySet(
         target, leaderContext, leader, leaderView, subject)

THEOREM AdequateLeaderCandidateRetirementEstablishesClosure ==
  \A target, leaderContext, leader, leaderView,
     subject, occurrenceRank, identity:
    AdequateLeaderTargetCandidateOwnerIdentityRetirementAction(
      target, leaderContext, leader, leaderView,
      subject, occurrenceRank, identity)
      => AdequateLeaderServicedCandidateClosure(
           target, leaderContext, leader, leaderView,
           subject, occurrenceRank, identity)'
BY Isa
   DEF AdequateLeaderServicedCandidateClosure,
       AdequateLeaderTargetCandidateOwnerIdentityRetirementAction,
       AdequateLeaderTargetOwnerIdentityRetirementAction,
       AdequateLeaderTargetLiveOwnerIdentitySet

THEOREM AdequateLeaderCandidateSuccessfulServiceRetirementStartsClosedMemory ==
  \A target, leaderContext, leader, leaderView,
     subject, occurrenceRank, identity:
    /\ AsyncCandidateServiceTombstoneLifecycleInvariant
    /\ AdequateLeaderTargetCandidateSuccessfulServiceRetirementAction(
         target, leaderContext, leader, leaderView,
         subject, occurrenceRank, identity)
    => /\ AdequateLeaderServicedCandidateMemory(
             target, leaderContext, leader, leaderView, subject, identity)'
       /\ AdequateLeaderServicedCandidateClosure(
            target, leaderContext, leader, leaderView,
            subject, occurrenceRank, identity)'
BY AdequateLeaderCandidateSuccessfulServiceRetirementInstallsServicedMemory,
   AdequateLeaderCandidateRetirementEstablishesClosure, Isa
   DEF AdequateLeaderTargetCandidateSuccessfulServiceRetirementAction

THEOREM AdequateLeaderCandidateTerminalRetirementStartsClosedMemory ==
  \A target, leaderContext, leader, leaderView,
     subject, occurrenceRank, identity:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateServiceTombstoneLifecycleInvariant
    /\ AdequateLeaderTargetCandidateTerminalDiscardRetirementAction(
         target, leaderContext, leader, leaderView,
         subject, occurrenceRank, identity)
    => /\ AdequateLeaderServicedCandidateMemory(
             target, leaderContext, leader, leaderView, subject, identity)'
       /\ AdequateLeaderServicedCandidateClosure(
            target, leaderContext, leader, leaderView,
            subject, occurrenceRank, identity)'
BY AdequateLeaderCandidateTerminalRetirementInstallsServicedMemory,
   AdequateLeaderCandidateRetirementEstablishesClosure, Isa
   DEF AdequateLeaderTargetCandidateTerminalDiscardRetirementAction

THEOREM AdequateLeaderServicedCandidateMemoryAndClosureAreStepInvariant ==
  \A target, leaderContext, leader, leaderView,
     subject, occurrenceRank, identity:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateServiceTombstoneLifecycleInvariant
    /\ identity \in
         AdequateLeaderFrozenCandidateOwnerUniverse(
           target, leaderContext, leader, leaderView, subject)
    /\ gst
    /\ AdequateLeaderServicedCandidateMemory(
         target, leaderContext, leader, leaderView, subject, identity)
    /\ AdequateLeaderServicedCandidateClosure(
         target, leaderContext, leader, leaderView,
         subject, occurrenceRank, identity)
    /\ AsyncNext
    => /\ AdequateLeaderServicedCandidateMemory(
             target, leaderContext, leader, leaderView, subject, identity)'
       /\ AdequateLeaderServicedCandidateClosure(
            target, leaderContext, leader, leaderView,
            subject, occurrenceRank, identity)'
BY AsyncNextPreservesCandidateServiceTombstoneLifecycle,
   AsyncCandidateServicedMarkerPersistsWithoutExit,
   AsyncCandidateTerminalTombstonePersistsWithoutExit,
   AsyncCandidateSameGenerationSuccessfulServiceIdentityPersistsUntilStrictExit,
   AdequateLeaderServicedCandidateIdentityHasServiceWitness,
   AdequateLeaderLiveAndServicedCandidateIdentitiesAreDisjoint,
   AdequateLeaderOwnerIdentityDeterminesNetworkServiceIdentity,
   IsaT(600)
   DEF AdequateLeaderServicedCandidateClosure,
       AdequateLeaderServicedCandidateMemory,
       AdequateLeaderTargetStrictOccurrenceDescentGoal,
       AdequateLeaderTargetLiveCandidateOwnerIdentitySet,
       AdequateLeaderTargetServicedCandidateOwnerIdentitySet,
       AdequateLeaderTargetCandidateIdentity,
       AdequateLeaderFrozenTargetCandidateIdentity,
       AdequateLeaderFrozenTargetCorridor,
       AdequateLeaderTargetCandidateRole,
       AsyncCandidateServiceTombstoneLifecycleInvariant,
       AsyncCandidateServiceLifecycleInvariant,
       AsyncCandidateTerminalTombstoneActive,
       AsyncCandidateTerminalTombstoneExitThisStep,
       AsyncCandidateServiceActiveTombstone,
       AsyncCandidateServiceExitThisStep,
       AsyncCandidateServiceTombstoned,
       AsyncCandidateServiceRecordsFor,
       AsyncCandidateServiceRecordsForIdentity,
       AsyncNext

AdequateLeaderTargetCandidateIdentityTombstoneProperty(specification) ==
  /\ AdequateLeaderCandidateFrozenIdentityBudgetBridgeProperty(
       specification)
  /\ (specification
        => \A target \in ValidatorIds,
              leaderContext \in ContextRecords,
              leader \in ValidatorIds,
              leaderView \in Views,
              subject \in Subjects,
              occurrenceRank \in
                AdequateLeaderTargetOccurrenceRankCarrier:
             /\ [](\A identity \in
                       AdequateLeaderFrozenCandidateOwnerUniverse(
                         target, leaderContext,
                         leader, leaderView, subject):
                      AdequateLeaderTargetCandidateServicedRetirementAction(
                        target, leaderContext, leader, leaderView,
                        subject, occurrenceRank, identity)
                        => /\ AdequateLeaderServicedCandidateMemory(
                                target, leaderContext, leader, leaderView,
                                subject, identity)'
                           /\ AdequateLeaderServicedCandidateClosure(
                                target, leaderContext, leader, leaderView,
                                subject, occurrenceRank, identity)')
                /\ [](\A identity \in
                          AdequateLeaderFrozenCandidateOwnerUniverse(
                            target, leaderContext,
                            leader, leaderView, subject):
                         /\ gst
                         /\ AdequateLeaderServicedCandidateMemory(
                              target, leaderContext, leader, leaderView,
                              subject, identity)
                         /\ AdequateLeaderServicedCandidateClosure(
                              target, leaderContext, leader, leaderView,
                              subject, occurrenceRank, identity)
                         => [](AdequateLeaderServicedCandidateMemory(
                                  target, leaderContext, leader, leaderView,
                                  subject, identity)
                                /\ AdequateLeaderServicedCandidateClosure(
                                     target, leaderContext, leader, leaderView,
                                     subject, occurrenceRank, identity))))

(***************************************************************************
Successful semantic service now retains its immutable process-generation
identity.  This property is intentionally limited to that concrete action:
the exact internal BodyAvailable fetch retirement uses the separate monotone
preflight property above, while any other proofless ignored occurrence remains
an honest separately bounded producer-episode residual.  Terminal internal
drains use the durable property below.
***************************************************************************)
AdequateLeaderTargetCandidateSuccessfulServiceMemoryProperty(
    specification) ==
  specification
    => \A target \in ValidatorIds,
          leaderContext \in ContextRecords,
          leader \in ValidatorIds,
          leaderView \in Views,
          subject \in Subjects,
          occurrenceRank \in
            AdequateLeaderTargetOccurrenceRankCarrier:
         [](\A identity \in
                  AdequateLeaderFrozenCandidateOwnerUniverse(
                    target, leaderContext,
                    leader, leaderView, subject):
              AdequateLeaderTargetCandidateSuccessfulServiceRetirementAction(
                target, leaderContext, leader, leaderView,
                subject, occurrenceRank, identity)
              => AdequateLeaderServicedCandidateMemory(
                   target, leaderContext, leader, leaderView,
                   subject, identity)')

AdequateLeaderTargetCandidateTerminalTombstoneProperty(specification) ==
  /\ AdequateLeaderCandidateFrozenIdentityBudgetBridgeProperty(
       specification)
  /\ (specification
        => \A target \in ValidatorIds,
              leaderContext \in ContextRecords,
              leader \in ValidatorIds,
              leaderView \in Views,
              subject \in Subjects,
              occurrenceRank \in
                AdequateLeaderTargetOccurrenceRankCarrier:
             /\ [](\A identity \in
                       AdequateLeaderFrozenCandidateOwnerUniverse(
                         target, leaderContext,
                         leader, leaderView, subject):
                      AdequateLeaderTargetCandidateTerminalDiscardRetirementAction(
                        target, leaderContext, leader, leaderView,
                        subject, occurrenceRank, identity)
                        => /\ AdequateLeaderServicedCandidateMemory(
                                target, leaderContext, leader, leaderView,
                                subject, identity)'
                           /\ AdequateLeaderServicedCandidateClosure(
                                target, leaderContext, leader, leaderView,
                                subject, occurrenceRank, identity)')
                /\ [](\A identity \in
                          AdequateLeaderFrozenCandidateOwnerUniverse(
                            target, leaderContext,
                            leader, leaderView, subject):
                         /\ gst
                         /\ AdequateLeaderServicedCandidateMemory(
                              target, leaderContext, leader, leaderView,
                              subject, identity)
                         /\ AdequateLeaderServicedCandidateClosure(
                              target, leaderContext, leader, leaderView,
                              subject, occurrenceRank, identity)
                         => [](AdequateLeaderServicedCandidateMemory(
                                  target, leaderContext, leader, leaderView,
                                  subject, identity)
                                /\ AdequateLeaderServicedCandidateClosure(
                                     target, leaderContext, leader, leaderView,
                                     subject, occurrenceRank, identity))))

THEOREM AsyncSpecProvidesAdequateLeaderTargetCandidateTerminalTombstones ==
  \A initialContext:
    AdequateLeaderTargetCandidateTerminalTombstoneProperty(
      AsyncSpecAt(initialContext))
BY AsyncSpecProvidesAdequateLeaderCandidateFrozenIdentityBudgetBridge,
   AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysProgressOwnershipInvariant,
   AsyncSpecAlwaysCandidateServiceTombstoneLifecycle,
   AdequateLeaderCandidateTerminalRetirementStartsClosedMemory,
   AdequateLeaderCandidateRetirementEstablishesClosure,
   AdequateLeaderServicedCandidateMemoryAndClosureAreStepInvariant,
   Isa, PTL
   DEF AdequateLeaderTargetCandidateTerminalTombstoneProperty,
       AsyncAllVars

THEOREM AsyncSpecProvidesAdequateLeaderTargetCandidateSuccessfulServiceMemory ==
  \A initialContext:
    AdequateLeaderTargetCandidateSuccessfulServiceMemoryProperty(
      AsyncSpecAt(initialContext))
BY AsyncSpecAlwaysCandidateServiceTombstoneLifecycle,
   AdequateLeaderCandidateSuccessfulServiceRetirementInstallsServicedMemory,
   Isa, PTL
   DEF AdequateLeaderTargetCandidateSuccessfulServiceMemoryProperty,
       AsyncAllVars

THEOREM AsyncSpecProvidesAdequateLeaderTargetCandidateIdentityTombstones ==
  \A initialContext:
    AdequateLeaderTargetCandidateIdentityTombstoneProperty(
      AsyncSpecAt(initialContext))
BY AsyncSpecProvidesAdequateLeaderCandidateFrozenIdentityBudgetBridge,
   AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysProgressOwnershipInvariant,
   AsyncSpecAlwaysCandidateServiceTombstoneLifecycle,
   AdequateLeaderCandidateSuccessfulServiceRetirementStartsClosedMemory,
   AdequateLeaderCandidateTerminalRetirementStartsClosedMemory,
   AdequateLeaderServicedCandidateMemoryAndClosureAreStepInvariant,
   Isa, PTL
   DEF AdequateLeaderTargetCandidateIdentityTombstoneProperty,
       AdequateLeaderTargetCandidateServicedRetirementAction,
       AsyncAllVars

THEOREM AdequateLeaderTargetNonDescentActionIntroducesOwnerIdentity ==
  \A target, leaderContext, leader, leaderView, subject, rank:
    AdequateLeaderTargetNonDescentEpisodeAction(
      target, leaderContext, leader, leaderView, subject, rank)
      => AdequateLeaderTargetRankIntroducedOwnerIdentitySet(
           target, leaderContext, leader,
           leaderView, subject, rank) # {}
BY AdequateLeaderTargetEqualCountReplacementIntroducesAndRetires,
   AdequateLeaderTargetCountIncreaseIntroducesOwnerIdentity, Isa
   DEF AdequateLeaderTargetNonDescentEpisodeAction

THEOREM AdequateLeaderTargetNonDescentIntroducedOwnersAreFrozen ==
  \A target, leaderContext, leader, leaderView, subject, rank:
    /\ AsyncStrongTypeInvariant'
    /\ AdequateLeaderTargetNonDescentEpisodeAction(
         target, leaderContext, leader, leaderView, subject, rank)
    => /\ AdequateLeaderTargetRankIntroducedOwnerIdentitySet(
             target, leaderContext, leader,
             leaderView, subject, rank) # {}
       /\ AdequateLeaderTargetRankIntroducedOwnerIdentitySet(
             target, leaderContext, leader,
             leaderView, subject, rank)
            \subseteq
              AdequateLeaderFrozenOwnerUniverse(
                target, leaderContext, leader, leaderView, subject)
BY AdequateLeaderTargetNonDescentActionIntroducesOwnerIdentity,
   AdequateLeaderFrozenTargetCandidatePayloadIsInStaticCarrier,
   AdequateLeaderFrozenCandidateOwnerIdentitySeparatesPayload,
   AdequateLeaderFrozenCandidateOwnerIdentityIsInjective,
   IsaT(300)
   DEF AdequateLeaderTargetRankIntroducedOwnerIdentitySet,
       AdequateLeaderTargetRankOwnerIdentitySet,
       AdequateLeaderTargetRankOwnerSet,
       AdequateLeaderTargetCandidateIdentity,
       AdequateLeaderFrozenCandidateOwnerIdentity,
       AdequateLeaderFrozenCandidateOwnerUniverse,
       AdequateLeaderFrozenOwnerUniverse

AdequateLeaderTargetNonDescentEpisodeResidual(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank, known) ==
  /\ AdequateLeaderTargetEpisodeKnownOwnerSet(
       target, leaderContext, leader, leaderView, subject, known)
  /\ ~AdequateLeaderTargetStrictOccurrenceDescentGoal(
       target, leaderContext, leader,
       leaderView, subject, sourceOccurrenceRank)
  /\ AdequateLeaderTargetOccurrenceEpisodeActive(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank)
  /\ AdequateLeaderTargetNonDescentDiscoveredOwnerIdentitySet(
       target, leaderContext, leader, leaderView, subject, known) # {}

AdequateLeaderTargetNonDescentEpisodeFrontier(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank, known) ==
  /\ AdequateLeaderTargetEpisodeKnownOwnerSet(
       target, leaderContext, leader, leaderView, subject, known)
  /\ ~AdequateLeaderTargetStrictOccurrenceDescentGoal(
       target, leaderContext, leader,
       leaderView, subject, sourceOccurrenceRank)
  /\ AdequateLeaderTargetOccurrenceEpisodeActive(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank)
  /\ AdequateLeaderTargetLiveOwnerIdentitySet(
       target, leaderContext, leader, leaderView, subject)
       \subseteq known

AdequateLeaderTargetNonDescentEpisodeAtBudget(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank, known, budget) ==
  /\ AdequateLeaderTargetNonDescentEpisodeFrontier(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank, known)
  /\ budget =
       AdequateLeaderTargetNonDescentEpisodeBudget(
         target, leaderContext, leader, leaderView, subject, known)

THEOREM AdequateLeaderTargetNonDescentEpisodeBudgetIsFiniteAndCoalesced ==
  \A target, leaderContext, leader, leaderView, subject, known:
    /\ AsyncStrongTypeInvariant
    /\ AdequateLeaderFrozenTargetCorridor(
         target, leaderContext, leader, leaderView)
    /\ AdequateLeaderTargetEpisodeKnownOwnerSet(
         target, leaderContext, leader, leaderView, subject, known)
    => /\ AdequateLeaderTargetNonDescentEpisodeBudget(
            target, leaderContext, leader, leaderView,
            subject, known) \in Nat
       /\ AdequateLeaderTargetNonDescentEpisodeBudget(
            target, leaderContext, leader, leaderView, subject, known)
            <= Cardinality(
                 AdequateLeaderFrozenOwnerUniverse(
                   target, leaderContext, leader, leaderView, subject))
BY AdequateLeaderFrozenOwnerUniverseIsFinite,
   FS_Subset, FS_CardinalityType, IsaT(180)
   DEF AdequateLeaderTargetNonDescentEpisodeBudget,
       AdequateLeaderTargetEpisodeKnownOwnerSet

AdequateLeaderTargetNonDescentKnownAdvanceGoal(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank, known, budget) ==
  \E discovered,
     known2 \in
       SUBSET AdequateLeaderFrozenOwnerUniverse(
         target, leaderContext, leader, leaderView, subject),
     budget2 \in Nat:
    /\ discovered =
         AdequateLeaderTargetNonDescentDiscoveredOwnerIdentitySet(
           target, leaderContext, leader, leaderView, subject, known)
    /\ discovered # {}
    /\ known2 = known \cup discovered
    /\ AdequateLeaderTargetNonDescentEpisodeAtBudget(
         target, leaderContext, leader, leaderView,
         subject, sourceOccurrenceRank, known2, budget2)
    /\ budget2 < budget

THEOREM AdequateLeaderTargetNonDescentDiscoveryStrictlyConsumesBudget ==
  \A target, leaderContext, leader, leaderView,
     subject, sourceOccurrenceRank, known, budget:
    /\ AsyncStrongTypeInvariant
    /\ AdequateLeaderTargetNonDescentEpisodeResidual(
         target, leaderContext, leader, leaderView,
         subject, sourceOccurrenceRank, known)
    /\ budget =
         AdequateLeaderTargetNonDescentEpisodeBudget(
           target, leaderContext, leader, leaderView, subject, known)
    => AdequateLeaderTargetNonDescentKnownAdvanceGoal(
         target, leaderContext, leader, leaderView,
         subject, sourceOccurrenceRank, known, budget)
BY AdequateLeaderFrozenOwnerUniverseIsFinite,
   AdequateLeaderLiveOwnersStayInsideFrozenUniverse,
   FS_Union, FS_Subset, FS_CardinalityType, IsaT(300)
   DEF AdequateLeaderTargetNonDescentDiscoveredOwnerIdentitySet,
       AdequateLeaderTargetNonDescentEpisodeResidual,
       AdequateLeaderTargetNonDescentEpisodeFrontier,
       AdequateLeaderTargetNonDescentEpisodeAtBudget,
       AdequateLeaderTargetNonDescentKnownAdvanceGoal,
       AdequateLeaderTargetNonDescentEpisodeBudget,
       AdequateLeaderTargetEpisodeKnownOwnerSet

THEOREM AdequateLeaderTargetNonDescentResidualAdvancesKnownBudget ==
  \A target, leaderContext, leader, leaderView,
     subject, sourceOccurrenceRank, known, budget:
    /\ AsyncStrongTypeInvariant
    /\ AdequateLeaderFrozenTargetCorridor(
         target, leaderContext, leader, leaderView)
    /\ AdequateLeaderTargetNonDescentEpisodeResidual(
         target, leaderContext, leader, leaderView,
         subject, sourceOccurrenceRank, known)
    /\ budget =
         AdequateLeaderTargetNonDescentEpisodeBudget(
           target, leaderContext, leader, leaderView, subject, known)
    => AdequateLeaderTargetNonDescentKnownAdvanceGoal(
         target, leaderContext, leader, leaderView,
         subject, sourceOccurrenceRank, known, budget)
BY AdequateLeaderTargetNonDescentDiscoveryStrictlyConsumesBudget

THEOREM AdequateLeaderTargetCurrentOwnersInitializeKnownEpisode ==
  \A target, leaderContext, leader, leaderView, subject:
    /\ AsyncStrongTypeInvariant
    /\ AdequateLeaderFrozenTargetCorridor(
         target, leaderContext, leader, leaderView)
    => AdequateLeaderTargetEpisodeStartsWithCurrentOwners(
         target, leaderContext, leader, leaderView, subject,
         AdequateLeaderTargetLiveOwnerIdentitySet(
           target, leaderContext, leader, leaderView, subject))
BY AdequateLeaderFrozenOwnerUniverseIsFinite,
   AdequateLeaderLiveOwnersStayInsideFrozenUniverse,
   FS_Subset, IsaT(180)
   DEF AdequateLeaderTargetEpisodeStartsWithCurrentOwners,
       AdequateLeaderTargetEpisodeKnownOwnerSet

\* This is the exact one-step occurrence mapping.  It does not call either
\* non-descent action progress: relative to the owners frozen before the step,
\* the post-state merely exposes a new finite-universe identity.  The separate
\* temporal episode must consume that identity.  Terminal discards have the
\* finite route-neutral tombstone table proved above.  Exact BodyAvailable
\* fetch retirement has the monotone preflight bridge above; successful
\* service has transient memory.  Any remaining proofless ignored retirement
\* is still an explicit residual and cannot be used to exclude a later A after
\* an equal-rank B replacement.
THEOREM AdequateLeaderTargetNonDescentActionExposesFreshEpisodeIdentity ==
  \A target, leaderContext, leader, leaderView, subject,
     sourceOccurrenceRank, known:
    /\ AsyncStrongTypeInvariant
    /\ AsyncStrongTypeInvariant'
    /\ AdequateLeaderTargetOccurrenceRankFrontier(
         target, leaderContext, leader, leaderView,
         subject, sourceOccurrenceRank)
    /\ AdequateLeaderTargetEpisodeStartsWithCurrentOwners(
         target, leaderContext, leader, leaderView, subject, known)
    /\ AdequateLeaderTargetNonDescentEpisodeAction(
         target, leaderContext, leader, leaderView,
         subject, sourceOccurrenceRank[1])
    /\ ~AdequateLeaderTargetStrictOccurrenceDescentGoal(
         target, leaderContext, leader, leaderView,
         subject, sourceOccurrenceRank)'
    => AdequateLeaderTargetNonDescentEpisodeResidual(
         target, leaderContext, leader, leaderView,
         subject, sourceOccurrenceRank, known)'
BY AdequateLeaderTargetNonDescentIntroducedOwnersAreFrozen,
   AdequateLeaderFrozenOwnerUniverseIsPrimeInvariant,
   AdequateLeaderLiveOwnersStayInsideFrozenUniverse,
   FS_Subset, IsaT(600)
   DEF AdequateLeaderTargetNonDescentEpisodeResidual,
       AdequateLeaderTargetOccurrenceEpisodeActive,
       AdequateLeaderTargetSameOrHigherOccurrenceFrontier,
       AdequateLeaderTargetNonDescentEpisodeAction,
       AdequateLeaderTargetEqualCountOwnerReplacementAction,
       AdequateLeaderTargetCountIncreasingReplenishmentAction,
       AdequateLeaderTargetNonDescentDiscoveredOwnerIdentitySet,
       AdequateLeaderTargetEpisodeStartsWithCurrentOwners,
       AdequateLeaderTargetEpisodeKnownOwnerSet,
       AdequateLeaderTargetOccurrenceRankFrontier,
       AdequateLeaderTargetRankFrontier,
       AdequateLeaderTargetRankOwnerCount,
       AdequateLeaderTargetLiveOwnerIdentitySet,
       AdequateLeaderTargetLiveCandidateOwnerIdentitySet,
       AdequateLeaderTargetRankIntroducedOwnerIdentitySet

AdequateLeaderTargetNonDescentFreshIdentityProperty(specification) ==
  specification
    => [](\A target \in ValidatorIds,
             leaderContext \in ContextRecords,
             leader \in ValidatorIds,
             leaderView \in Views,
             subject \in Subjects,
             rank \in AdequateLeaderTargetSemanticRankCarrier:
            AdequateLeaderTargetNonDescentEpisodeAction(
              target, leaderContext, leader, leaderView, subject, rank)
              => /\ AdequateLeaderTargetRankIntroducedOwnerIdentitySet(
                       target, leaderContext, leader,
                       leaderView, subject, rank) # {}
                 /\ AdequateLeaderTargetRankIntroducedOwnerIdentitySet(
                       target, leaderContext, leader,
                       leaderView, subject, rank)
                      \subseteq
                        AdequateLeaderFrozenOwnerUniverse(
                          target, leaderContext, leader,
                          leaderView, subject))

THEOREM AsyncLiveAdequateLeaderTargetNonDescentIntroducesFreshIdentity ==
  \A initialContext:
    AdequateLeaderTargetNonDescentFreshIdentityProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncSpecAlwaysStrongTypeInvariant,
   AsyncLiveSpecProjectsAsyncSpec,
   AsyncBracketNextPreservesStrongTypeInvariant,
   AdequateLeaderTargetNonDescentIntroducedOwnersAreFrozen,
   PTL
   DEF AdequateLeaderTargetNonDescentFreshIdentityProperty

(***************************************************************************
Owner-indexed off-subject retirement.

The earlier subject-switch seam selected an arbitrary closed control item
after occurrence service.  A retained old slot could therefore satisfy the
existential again after its identity had already entered `retired`, while a
different live candidate remained unretired.  The episode below instead
freezes one exact rank owner.  Service must close that same immutable identity
before it can consume one unit of the subject-switch budget.

Candidate service markers cover successful tracked service and eligible
terminal retirement.  A control Delivery candidate which is intentionally
discarded without application is covered by the retained exact/current slot
or its strictly-newer replacement.  An internal BodyAvailable callback whose
exact body has already left Missing is covered by the reducer-stage preflight
retirement above.  A drained producer continuation is covered only by its
exact Terminal record or a strictly-newer-view record at the same bounded
slot/stage address.  No other disappearance is accepted as a closed owner.
***************************************************************************)
AdequateLeaderTargetOccurrenceOwnerIdentitySet(
    target, leaderContext, leader, leaderView,
    subject, occurrenceRank) ==
  IF occurrenceRank \in AdequateLeaderTargetOccurrenceRankCarrier
  THEN AdequateLeaderTargetRankOwnerIdentitySet(
         target, leaderContext, leader, leaderView,
         subject, occurrenceRank[1])
  ELSE {}

AdequateLeaderTargetOccurrenceOwnerSelected(
    target, leaderContext, leader, leaderView,
    subject, occurrenceRank, owner) ==
  /\ AdequateLeaderTargetOccurrenceRankFrontier(
       target, leaderContext, leader, leaderView,
       subject, occurrenceRank)
  /\ owner \in
       AdequateLeaderTargetOccurrenceOwnerIdentitySet(
         target, leaderContext, leader, leaderView,
         subject, occurrenceRank)

AdequateLeaderTargetOccurrenceOwnerRetirementClosed(
    target, leaderContext, leader, leaderView,
    subject, occurrenceRank, owner) ==
  /\ owner \in
       AdequateLeaderFrozenCandidateOwnerUniverse(
         target, leaderContext, leader, leaderView, subject)
  /\ owner \notin
       AdequateLeaderTargetLiveCandidateOwnerIdentitySet(
         target, leaderContext, leader, leaderView, subject)
  /\ \/ owner \in
          AdequateLeaderTargetServicedCandidateOwnerIdentitySet(
            target, leaderContext, leader, leaderView, subject)
     \/ owner \in
          AdequateLeaderTargetInternalBodyAvailableRetiredOwnerIdentitySet(
            target, leaderContext, leader, leaderView, subject)
     \/ owner \in
          AdequateLeaderTargetProducerContinuationRetiredOwnerIdentitySet(
            target, leaderContext, leader, leaderView, subject)
     \/ owner \in
          AdequateLeaderTargetOffSubjectControlClosedOwnerIdentitySet(
            target, leaderContext, leader, leaderView,
            subject, occurrenceRank)
     \/ NodeHasDecision(owner.owner)

THEOREM AdequateLeaderSelectedOccurrenceOwnerIsFrozenAndLive ==
  \A target, leaderContext, leader, leaderView,
     subject, occurrenceRank, owner:
    /\ AsyncStrongTypeInvariant
    /\ AdequateLeaderFrozenTargetCorridor(
         target, leaderContext, leader, leaderView)
    /\ AdequateLeaderTargetOccurrenceOwnerSelected(
         target, leaderContext, leader, leaderView,
         subject, occurrenceRank, owner)
    => /\ owner \in
            AdequateLeaderFrozenCandidateOwnerUniverse(
              target, leaderContext, leader, leaderView, subject)
       /\ owner \in
            AdequateLeaderTargetLiveCandidateOwnerIdentitySet(
              target, leaderContext, leader, leaderView, subject)
BY AdequateLeaderLiveOwnersStayInsideFrozenUniverse, IsaT(300)
   DEF AdequateLeaderTargetOccurrenceOwnerSelected,
       AdequateLeaderTargetOccurrenceOwnerIdentitySet,
       AdequateLeaderTargetOccurrenceRankFrontier,
       AdequateLeaderTargetRankOwnerIdentitySet,
       AdequateLeaderTargetLiveOwnerIdentitySet,
       AdequateLeaderTargetLiveCandidateOwnerIdentitySet,
       AdequateLeaderFrozenOwnerUniverse,
       AdequateLeaderFrozenCandidateOwnerUniverse,
       AdequateLeaderFrozenWireOwnerUniverse

THEOREM AdequateLeaderOccurrenceFrontierHasSelectedFrozenOwner ==
  \A target, leaderContext, leader, leaderView,
     subject, occurrenceRank:
    /\ AsyncStrongTypeInvariant
    /\ AdequateLeaderFrozenTargetCorridor(
         target, leaderContext, leader, leaderView)
    /\ AdequateLeaderTargetOccurrenceRankFrontier(
         target, leaderContext, leader, leaderView,
         subject, occurrenceRank)
    => \E owner \in
         AdequateLeaderFrozenCandidateOwnerUniverse(
           target, leaderContext, leader, leaderView, subject):
         AdequateLeaderTargetOccurrenceOwnerSelected(
           target, leaderContext, leader, leaderView,
           subject, occurrenceRank, owner)
BY AdequateLeaderLiveOwnersStayInsideFrozenUniverse, IsaT(240)
   DEF AdequateLeaderTargetOccurrenceOwnerSelected,
       AdequateLeaderTargetOccurrenceOwnerIdentitySet,
       AdequateLeaderTargetOccurrenceRankFrontier,
       AdequateLeaderTargetRankFrontier,
       AdequateLeaderTargetRankOwnerIdentitySet,
       AdequateLeaderTargetRankOwnerSet,
       AdequateLeaderTargetCandidateIdentity,
       AdequateLeaderTargetLiveOwnerIdentitySet,
       AdequateLeaderTargetLiveCandidateOwnerIdentitySet

(***************************************************************************
Finite physical-continuation episode for one selected frozen owner.

`SameIdentityLeaderOwner` can already be true while the selected candidate is
still owned, so it cannot itself be a temporal endpoint.  The episode below
instead freezes the complete set of concrete candidate values which currently
carry the selected target/context/leader/view/subject/rank owner.  Every such
candidate has the same route-neutral service identity.  While the set is
nonempty, ordinary admission and causal production coalesce against that
identity; a real owner-exit step therefore cannot add a new member.

The frontier retains the initial candidate set as an immutable anchor while
its natural-number rank is the cardinality of the current subset.  The
terminal predicate requires that current set to be empty.  Its existential
can therefore select only a candidate which actually carried this owner at
the episode source, excluding both an already-true evidence shortcut and the
fabricated-stale-candidate shortcut.  `SameIdentity` is absent from the in-
corridor terminal: while the corridor persists it is represented only by
strict descent of the finite concrete-owner count.  It is retained as exact
lineage only when a separate corridor-exit handoff occurs.
***************************************************************************)
AdequateLeaderTargetSelectedOwnerCandidateSet(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank, owner) ==
  {candidate \in AsyncCandidateSet:
     /\ ResponsiveProtectedCandidateOwned(candidate)
     /\ AdequateLeaderFrozenTargetCandidateIdentity(
          candidate, sourceOccurrenceRank[1],
          target, leaderContext, leader, leaderView, subject)
     /\ owner =
          AdequateLeaderFrozenCandidateOwnerIdentity(
            candidate, sourceOccurrenceRank[1],
            target, leaderContext, leader, leaderView, subject)}

AdequateLeaderTargetSelectedOwnerPhysicalEpisodeFrontier(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank, owner,
    sourceCandidates, budget) ==
  /\ AdequateLeaderFrozenTargetCorridor(
       target, leaderContext, leader, leaderView)
  /\ sourceCandidates # {}
  /\ AdequateLeaderTargetSelectedOwnerCandidateSet(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank, owner)
       \subseteq sourceCandidates
  /\ AdequateLeaderTargetSelectedOwnerCandidateSet(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank, owner) # {}
  /\ budget \in Nat \ {0}
  /\ budget =
       Cardinality(
         AdequateLeaderTargetSelectedOwnerCandidateSet(
           target, leaderContext, leader, leaderView,
           subject, sourceOccurrenceRank, owner))

AdequateLeaderTargetSelectedOwnerNonContinuationTerminal(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank, owner, sourceCandidates) ==
  /\ AdequateLeaderFrozenTargetCorridor(
       target, leaderContext, leader, leaderView)
  /\ AdequateLeaderTargetSelectedOwnerCandidateSet(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank, owner) = {}
  /\ \E candidate \in sourceCandidates:
    /\ AdequateLeaderFrozenTargetCandidateIdentity(
         candidate, sourceOccurrenceRank[1],
         target, leaderContext, leader, leaderView, subject)
    /\ owner =
         AdequateLeaderFrozenCandidateOwnerIdentity(
           candidate, sourceOccurrenceRank[1],
           target, leaderContext, leader, leaderView, subject)

AdequateLeaderTargetSelectedOwnerPhysicalEpisodeCorridorTerminal(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank, owner, sourceCandidates) ==
  LET authority ==
        AdequateLeaderCorridorAuthorityReceipt(
          target, leaderContext, leader, leaderView)
  IN /\ gst
  /\ AdequateLeaderCorridorAuthorityReceiptValid(authority)
  /\ authority.context = context
  /\ owner.authority = authority
  /\ ~AdequateLeaderFrozenTargetCorridor(
       target, leaderContext, leader, leaderView)
  /\ \E candidate \in sourceCandidates:
       /\ AdequateLeaderFrozenTargetCandidateIdentity(
            candidate, sourceOccurrenceRank[1],
            target, leaderContext, leader, leaderView, subject)
       /\ owner =
            AdequateLeaderFrozenCandidateOwnerIdentity(
              candidate, sourceOccurrenceRank[1],
              target, leaderContext, leader, leaderView, subject)
       /\ \/ ResponsiveProtectedCandidateOwned(candidate)
          \/ NodeHasDecision(candidate.node)
          \/ NodeHasApplication(candidate.node)
          \/ ExactLeaderCandidatePostMilestone(
               candidate, sourceOccurrenceRank[1])
          \/ SameIdentityLeaderOwner(candidate)
          \/ ExactLeaderEvidenceAt(
               candidate, sourceOccurrenceRank[1])
          \/ ExactLeaderDiscardProvenanceAt(
               candidate, sourceOccurrenceRank[1])
          \/ AsyncCandidateServiceTombstoned(candidate)
          \/ AsyncCandidateLifecycleRecorded(
               candidate.node, candidate.causalOrigin)
          \/ /\ ~CandidateConsumerCurrent(candidate)
             /\ \/ candidate.consumerContext # context
                \/ \E installed \in installedTCs:
                     /\ installed.node = candidate.node
                     /\ installed.tc.context = candidate.consumerContext
                     /\ installed.tc.view >= candidate.consumerView

AdequateLeaderTargetSelectedOwnerPhysicalEpisodeTerminal(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank, owner, sourceCandidates) ==
  \/ AdequateLeaderTargetSelectedOwnerNonContinuationTerminal(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank, owner, sourceCandidates)
  \/ AdequateLeaderTargetSelectedOwnerPhysicalEpisodeCorridorTerminal(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank, owner, sourceCandidates)

AdequateLeaderTargetSelectedOwnerPhysicalEpisodeDescentGoal(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank, owner,
    sourceCandidates, budget) ==
  \/ AdequateLeaderTargetSelectedOwnerPhysicalEpisodeTerminal(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank, owner, sourceCandidates)
  \/ \E lowerBudget \in SetLessThan(
          budget, OpToRel(<, Nat), Nat):
       AdequateLeaderTargetSelectedOwnerPhysicalEpisodeFrontier(
         target, leaderContext, leader, leaderView,
         subject, sourceOccurrenceRank, owner,
         sourceCandidates, lowerBudget)

THEOREM AdequateLeaderSelectedOwnerCandidateSetIsFinite ==
  \A target, leaderContext, leader, leaderView,
     subject, sourceOccurrenceRank, owner:
    AsyncStrongTypeInvariant
      => IsFiniteSet(
           AdequateLeaderTargetSelectedOwnerCandidateSet(
             target, leaderContext, leader, leaderView,
             subject, sourceOccurrenceRank, owner))
BY StrongTypeHasFiniteHistoricalDiscoveryRankOwners,
   FS_Subset, Isa
   DEF AdequateLeaderTargetSelectedOwnerCandidateSet,
       ActiveScheduledCandidates,
       AdequateLeaderFrozenTargetCandidateIdentity,
       ExactLeaderFrozenSemanticIdentity,
       ExactLeaderStaticSemanticRank,
       ResponsiveProtectedCandidateOwned,
       ProtectedCandidateOwned, CandidateScheduled

THEOREM AdequateLeaderSelectedOwnerCandidatesShareServiceIdentity ==
  \A target, leaderContext, leader, leaderView,
     subject, sourceOccurrenceRank, owner,
     left, right:
    /\ left \in
         AdequateLeaderTargetSelectedOwnerCandidateSet(
           target, leaderContext, leader, leaderView,
           subject, sourceOccurrenceRank, owner)
    /\ right \in
         AdequateLeaderTargetSelectedOwnerCandidateSet(
           target, leaderContext, leader, leaderView,
           subject, sourceOccurrenceRank, owner)
    => AsyncCandidateServiceIdentity(left)
         = AsyncCandidateServiceIdentity(right)
BY AdequateLeaderOwnerIdentityDeterminesNetworkServiceIdentity,
   Isa
   DEF AdequateLeaderTargetSelectedOwnerCandidateSet,
       AdequateLeaderFrozenTargetCandidateIdentity

THEOREM AdequateLeaderSelectedOccurrenceOwnerStartsPhysicalEpisode ==
  \A target, leaderContext, leader, leaderView,
     subject, sourceOccurrenceRank, owner:
    /\ AsyncStrongTypeInvariant
    /\ AdequateLeaderFrozenTargetCorridor(
         target, leaderContext, leader, leaderView)
    /\ AdequateLeaderTargetOccurrenceOwnerSelected(
         target, leaderContext, leader, leaderView,
         subject, sourceOccurrenceRank, owner)
    => \E sourceCandidates \in SUBSET AsyncCandidateSet,
          budget \in Nat:
         AdequateLeaderTargetSelectedOwnerPhysicalEpisodeFrontier(
           target, leaderContext, leader, leaderView,
           subject, sourceOccurrenceRank, owner,
           sourceCandidates, budget)
BY AdequateLeaderSelectedOwnerCandidateSetIsFinite,
   AdequateLeaderSelectedOccurrenceOwnerIsFrozenAndLive,
   FS_CardinalityType, IsaT(300)
   DEF AdequateLeaderTargetOccurrenceOwnerSelected,
       AdequateLeaderTargetOccurrenceOwnerIdentitySet,
       AdequateLeaderTargetRankOwnerIdentitySet,
       AdequateLeaderTargetRankOwnerSet,
       AdequateLeaderTargetSelectedOwnerCandidateSet,
       AdequateLeaderTargetSelectedOwnerPhysicalEpisodeFrontier,
       AdequateLeaderTargetCandidateIdentity,
       AdequateLeaderFrozenTargetCandidateIdentity,
       AdequateLeaderTargetLiveCandidateOwnerIdentitySet

THEOREM AdequateLeaderSelectedOwnerCandidateSetCannotReplenish ==
  \A target, leaderContext, leader, leaderView,
     subject, sourceOccurrenceRank, owner:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateServiceTombstoneLifecycleInvariant
    /\ gst
    /\ AdequateLeaderTargetSelectedOwnerCandidateSet(
         target, leaderContext, leader, leaderView,
         subject, sourceOccurrenceRank, owner) # {}
    /\ [AsyncNext]_AsyncAllVars
    => AdequateLeaderTargetSelectedOwnerCandidateSet(
         target, leaderContext, leader, leaderView,
         subject, sourceOccurrenceRank, owner)'
         \subseteq
       AdequateLeaderTargetSelectedOwnerCandidateSet(
         target, leaderContext, leader, leaderView,
         subject, sourceOccurrenceRank, owner)
BY AdequateLeaderSelectedOwnerCandidatesShareServiceIdentity,
   IsaT(1800)
   DEF AdequateLeaderTargetSelectedOwnerCandidateSet,
       AdequateLeaderFrozenTargetCandidateIdentity,
       ExactLeaderFrozenSemanticIdentity,
       ExactLeaderStaticSemanticRank,
       AsyncCandidateServiceTombstoneLifecycleInvariant,
       AsyncCandidateServiceLifecycleInvariant,
       AsyncCandidateLifecycleStageIdentityInvariant,
       ResponsiveProtectedCandidateOwned,
       ProtectedCandidateOwned,
       CandidateAdmissionCoalesced, FreshCandidateSequence,
       EnqueueCandidate, CandidateScheduled,
       CandidateScheduledAfter, AsyncNext, AsyncAllVars

THEOREM AdequateLeaderSelectedOwnerPhysicalEpisodeStepIsTerminalDescentOrFrame ==
  \A target, leaderContext, leader, leaderView,
     subject, sourceOccurrenceRank, owner,
     sourceCandidates, budget:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateServiceTombstoneLifecycleInvariant
    /\ AdequateLeaderTargetSelectedOwnerPhysicalEpisodeFrontier(
         target, leaderContext, leader, leaderView,
         subject, sourceOccurrenceRank, owner,
         sourceCandidates, budget)
    /\ [AsyncNext]_AsyncAllVars
    => \/ AdequateLeaderTargetSelectedOwnerPhysicalEpisodeDescentGoal(
             target, leaderContext, leader, leaderView,
             subject, sourceOccurrenceRank, owner,
             sourceCandidates, budget)'
       \/ AdequateLeaderTargetSelectedOwnerPhysicalEpisodeFrontier(
             target, leaderContext, leader, leaderView,
             subject, sourceOccurrenceRank, owner,
             sourceCandidates, budget)'
BY AdequateLeaderSelectedOwnerCandidateSetCannotReplenish,
   AdequateLeaderSelectedOwnerCandidateSetIsFinite,
   AsyncCandidateScheduledIdentityDepartureRetiresLifecycleAtGst,
   FS_CardinalityType, IsaT(2400)
   DEF AdequateLeaderTargetSelectedOwnerCandidateSet,
       AdequateLeaderTargetSelectedOwnerPhysicalEpisodeFrontier,
       AdequateLeaderTargetSelectedOwnerPhysicalEpisodeDescentGoal,
       AdequateLeaderTargetSelectedOwnerPhysicalEpisodeTerminal,
       AdequateLeaderTargetSelectedOwnerNonContinuationTerminal,
       AdequateLeaderTargetSelectedOwnerPhysicalEpisodeCorridorTerminal,
       SetLessThan, OpToRel, AsyncAllVars

AdequateLeaderTargetSelectedOwnerPhysicalEpisodeRankStepProperty(
    specification) ==
  specification
    => \A target \in ValidatorIds,
          leaderContext \in ContextRecords,
          leader \in ValidatorIds,
          leaderView \in Views,
          subject \in Subjects,
          sourceOccurrenceRank \in
            AdequateLeaderTargetOccurrenceRankCarrier,
          owner \in
            AdequateLeaderFrozenCandidateOwnerUniverse(
              target, leaderContext, leader, leaderView, subject),
          sourceCandidates \in SUBSET AsyncCandidateSet,
          budget \in Nat:
         AdequateLeaderTargetSelectedOwnerPhysicalEpisodeFrontier(
           target, leaderContext, leader, leaderView,
           subject, sourceOccurrenceRank, owner,
           sourceCandidates, budget)
           ~> AdequateLeaderTargetSelectedOwnerPhysicalEpisodeDescentGoal(
                target, leaderContext, leader, leaderView,
                subject, sourceOccurrenceRank, owner,
                sourceCandidates, budget)

AdequateLeaderTargetSelectedOwnerPhysicalEpisodeClosureProperty(
    specification) ==
  specification
    => \A target \in ValidatorIds,
          leaderContext \in ContextRecords,
          leader \in ValidatorIds,
          leaderView \in Views,
          subject \in Subjects,
          sourceOccurrenceRank \in
            AdequateLeaderTargetOccurrenceRankCarrier,
          owner \in
            AdequateLeaderFrozenCandidateOwnerUniverse(
              target, leaderContext, leader, leaderView, subject),
          sourceCandidates \in SUBSET AsyncCandidateSet,
          budget \in Nat:
         AdequateLeaderTargetSelectedOwnerPhysicalEpisodeFrontier(
           target, leaderContext, leader, leaderView,
           subject, sourceOccurrenceRank, owner,
           sourceCandidates, budget)
           ~> AdequateLeaderTargetSelectedOwnerPhysicalEpisodeTerminal(
                target, leaderContext, leader, leaderView,
                subject, sourceOccurrenceRank, owner, sourceCandidates)

THEOREM AsyncLiveProvidesAdequateLeaderSelectedOwnerPhysicalEpisodeRankStep ==
  \A initialContext:
    AdequateLeaderTargetSelectedOwnerPhysicalEpisodeRankStepProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncSpecProvidesProtectedServiceFiniteRunnerEpisodeClosure,
   StarvationFreedomObligation,
   AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysProgressOwnershipInvariant,
   AsyncSpecAlwaysCandidateServiceTombstoneLifecycle,
   AsyncLiveSpecProjectsAsyncSpec,
   AdequateLeaderSelectedOwnerPhysicalEpisodeStepIsTerminalDescentOrFrame,
   PTL, IsaT(1200)
   DEF StarvationFreedomProperty,
       AdequateLeaderTargetSelectedOwnerCandidateSet,
       AdequateLeaderTargetSelectedOwnerPhysicalEpisodeFrontier,
       AdequateLeaderTargetSelectedOwnerPhysicalEpisodeDescentGoal,
       AdequateLeaderTargetSelectedOwnerPhysicalEpisodeRankStepProperty

THEOREM AsyncLiveProvidesAdequateLeaderSelectedOwnerPhysicalEpisodeClosure ==
  \A initialContext:
    AdequateLeaderTargetSelectedOwnerPhysicalEpisodeClosureProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncLiveProvidesAdequateLeaderSelectedOwnerPhysicalEpisodeRankStep,
   NatLessThanWellFounded, WellFoundedLeadsTo
   DEF AdequateLeaderTargetSelectedOwnerPhysicalEpisodeRankStepProperty,
       AdequateLeaderTargetSelectedOwnerPhysicalEpisodeClosureProperty,
       AdequateLeaderTargetSelectedOwnerPhysicalEpisodeDescentGoal

AdequateLeaderTargetSelectedOwnerPhysicalOutcomeProperty(specification) ==
  specification
    => \A target \in ValidatorIds,
          leaderContext \in ContextRecords,
          leader \in ValidatorIds,
          leaderView \in Views,
          subject \in Subjects,
          sourceOccurrenceRank \in
            AdequateLeaderTargetOccurrenceRankCarrier,
          owner \in
            AdequateLeaderFrozenCandidateOwnerUniverse(
              target, leaderContext, leader, leaderView, subject):
         AdequateLeaderTargetOccurrenceOwnerSelected(
           target, leaderContext, leader, leaderView,
           subject, sourceOccurrenceRank, owner)
           ~> \E sourceCandidates \in SUBSET AsyncCandidateSet:
                AdequateLeaderTargetSelectedOwnerPhysicalEpisodeTerminal(
                  target, leaderContext, leader, leaderView,
                  subject, sourceOccurrenceRank, owner, sourceCandidates)

THEOREM AsyncLiveProvidesAdequateLeaderSelectedOwnerPhysicalOutcome ==
  \A initialContext:
    AdequateLeaderTargetSelectedOwnerPhysicalOutcomeProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncSpecAlwaysStrongTypeInvariant,
   AsyncLiveSpecProjectsAsyncSpec,
   AdequateLeaderSelectedOccurrenceOwnerStartsPhysicalEpisode,
   AsyncLiveProvidesAdequateLeaderSelectedOwnerPhysicalEpisodeClosure,
   PTL, IsaT(600)
   DEF AdequateLeaderTargetSelectedOwnerPhysicalOutcomeProperty,
       AdequateLeaderTargetSelectedOwnerPhysicalEpisodeClosureProperty

THEOREM AdequateLeaderSelectedOwnerNonContinuationDrainsExactLiveOwner ==
  \A target, leaderContext, leader, leaderView,
     subject, sourceOccurrenceRank, owner, sourceCandidates:
    AdequateLeaderTargetSelectedOwnerNonContinuationTerminal(
      target, leaderContext, leader, leaderView,
      subject, sourceOccurrenceRank, owner, sourceCandidates)
      => owner \notin
           AdequateLeaderTargetLiveCandidateOwnerIdentitySet(
             target, leaderContext, leader, leaderView, subject)
BY IsaT(300)
   DEF AdequateLeaderTargetSelectedOwnerNonContinuationTerminal,
       AdequateLeaderTargetSelectedOwnerCandidateSet,
       AdequateLeaderTargetLiveCandidateOwnerIdentitySet,
       AdequateLeaderTargetRankOwnerIdentitySet,
       AdequateLeaderTargetRankOwnerSet,
       AdequateLeaderTargetCandidateIdentity,
       AdequateLeaderFrozenTargetCandidateIdentity,
       AdequateLeaderFrozenCandidateOwnerIdentity,
       ResponsiveProtectedCandidateOwned

(***************************************************************************
Durable finite subject-switch memory.

`retired` is not a freely chosen temporal-history ghost.  It is the exact
projection of the existing candidate-service tombstones, producer-
continuation Terminal/newer-view high-water memory, monotone internal
BodyAvailable preflight retirements, and retained control-slot retirement
records over the frozen target/context/leader/view universe.  A candidate
owned by a node with a durable Decision is retired by that Decision record
even if its transient candidate tombstone is reclaimed; this does not treat
the other target as decided.  Thus productive re-entry observes the same
accumulated set from Async state.  No new wire field, environment knob, or
unbounded history variable is introduced.
***************************************************************************)
AdequateLeaderTargetDurablyRetiredOwnerIdentitySet(
    target, leaderContext, leader, leaderView) ==
  {identity \in
     AdequateLeaderFrozenSubjectSwitchOwnerUniverse(
       target, leaderContext, leader, leaderView):
     \E subject \in Subjects:
       \/ identity \in
            AdequateLeaderTargetServicedCandidateOwnerIdentitySet(
              target, leaderContext, leader, leaderView, subject)
       \/ identity \in
            AdequateLeaderTargetInternalBodyAvailableRetiredOwnerIdentitySet(
              target, leaderContext, leader, leaderView, subject)
       \/ identity \in
            AdequateLeaderTargetProducerContinuationRetiredOwnerIdentitySet(
              target, leaderContext, leader, leaderView, subject)
       \/ \E occurrenceRank \in
              AdequateLeaderTargetOccurrenceRankCarrier:
            identity \in
              AdequateLeaderTargetOffSubjectControlClosedOwnerIdentitySet(
                target, leaderContext, leader, leaderView,
                subject, occurrenceRank)
       \/ /\ identity \in
               AdequateLeaderFrozenCandidateOwnerUniverse(
                 target, leaderContext, leader, leaderView, subject)
          /\ NodeHasDecision(identity.owner)}

AdequateLeaderTargetSubjectSwitchRetiredOwnerSet(
    target, leaderContext, leader, leaderView, retired) ==
  /\ retired =
       AdequateLeaderTargetDurablyRetiredOwnerIdentitySet(
         target, leaderContext, leader, leaderView)
  /\ IsFiniteSet(retired)
  /\ retired \subseteq
       AdequateLeaderFrozenSubjectSwitchOwnerUniverse(
         target, leaderContext, leader, leaderView)

AdequateLeaderTargetSubjectSwitchRemainingBudget(
    target, leaderContext, leader, leaderView, retired) ==
  Cardinality(
    AdequateLeaderFrozenSubjectSwitchOwnerUniverse(
      target, leaderContext, leader, leaderView)
      \ retired)

THEOREM AdequateLeaderClosedOccurrenceOwnerIsDurablyRetired ==
  \A target, leaderContext, leader, leaderView,
     subject, occurrenceRank, owner:
    AdequateLeaderTargetOccurrenceOwnerRetirementClosed(
      target, leaderContext, leader, leaderView,
      subject, occurrenceRank, owner)
      => owner \in
           AdequateLeaderTargetDurablyRetiredOwnerIdentitySet(
             target, leaderContext, leader, leaderView)
BY Isa
   DEF AdequateLeaderTargetOccurrenceOwnerRetirementClosed,
       AdequateLeaderTargetDurablyRetiredOwnerIdentitySet,
       AdequateLeaderFrozenSubjectSwitchOwnerUniverse,
       AdequateLeaderFrozenOwnerUniverse

THEOREM AdequateLeaderDurablyRetiredOwnerHasExistingClosureWitness ==
  \A target, leaderContext, leader, leaderView, owner:
    owner \in
      AdequateLeaderTargetDurablyRetiredOwnerIdentitySet(
        target, leaderContext, leader, leaderView)
      => /\ owner \in
              AdequateLeaderFrozenSubjectSwitchOwnerUniverse(
                target, leaderContext, leader, leaderView)
         /\ \E subject \in Subjects:
              \/ owner \in
                   AdequateLeaderTargetServicedCandidateOwnerIdentitySet(
                     target, leaderContext, leader, leaderView, subject)
              \/ owner \in
                   AdequateLeaderTargetInternalBodyAvailableRetiredOwnerIdentitySet(
                     target, leaderContext, leader, leaderView, subject)
              \/ owner \in
                   AdequateLeaderTargetProducerContinuationRetiredOwnerIdentitySet(
                     target, leaderContext, leader, leaderView, subject)
              \/ \E occurrenceRank \in
                     AdequateLeaderTargetOccurrenceRankCarrier:
                   owner \in
                     AdequateLeaderTargetOffSubjectControlClosedOwnerIdentitySet(
                       target, leaderContext, leader, leaderView,
                       subject, occurrenceRank)
              \/ /\ owner \in
                      AdequateLeaderFrozenCandidateOwnerUniverse(
                        target, leaderContext, leader, leaderView, subject)
                 /\ NodeHasDecision(owner.owner)
BY Isa
   DEF AdequateLeaderTargetDurablyRetiredOwnerIdentitySet

THEOREM AdequateLeaderDurablyRetiredOwnersAreNotLiveInFrozenCorridor ==
  \A target, leaderContext, leader, leaderView, subject:
    /\ AsyncStrongTypeInvariant
    /\ AsyncCandidateServiceTombstoneLifecycleInvariant
    /\ AdequateLeaderFrozenTargetCorridor(
         target, leaderContext, leader, leaderView)
    => AdequateLeaderTargetDurablyRetiredOwnerIdentitySet(
         target, leaderContext, leader, leaderView)
         \cap
       AdequateLeaderTargetLiveCandidateOwnerIdentitySet(
         target, leaderContext, leader, leaderView, subject)
         = {}
BY AdequateLeaderLiveAndServicedCandidateIdentitiesAreDisjoint,
   AdequateLeaderInternalBodyAvailableRetiredOwnersAreNotLive,
   AdequateLeaderProducerContinuationRetiredOwnersAreNotLive,
   IsaT(900)
   DEF AdequateLeaderTargetDurablyRetiredOwnerIdentitySet,
       AdequateLeaderTargetProducerContinuationRetiredOwnerIdentitySet,
       AdequateLeaderTargetOffSubjectControlClosedOwnerIdentitySet,
       AdequateLeaderTargetOffSubjectControlRetirementClosed,
       AdequateLeaderTargetOffSubjectControlRetirementMemory,
       AdequateLeaderTargetOffSubjectControlCandidateOwnerIdentity,
       AdequateLeaderTargetOffSubjectControlOccurrenceIdentity,
       AdequateLeaderTargetLiveCandidateOwnerIdentitySet,
       AdequateLeaderTargetRankOwnerIdentitySet,
       AdequateLeaderTargetRankOwnerSet,
       AdequateLeaderTargetCandidateIdentity,
       AdequateLeaderFrozenTargetCandidateIdentity,
       AdequateLeaderTargetCandidateRole,
       AdequateLeaderFrozenTargetCandidateRole,
       AdequateLeaderFrozenCandidateOwnerIdentity,
       AdequateLeaderFrozenCandidateOwnerIdentityFromPayload,
       AdequateLeaderFrozenWireOwnerIdentity,
       AdequateLeaderFrozenWireOwnerIdentityFromCoordinates,
       AdequateLeaderFrozenOwnerUniverse,
       AdequateLeaderFrozenCandidateOwnerUniverse,
       AdequateLeaderFrozenWireOwnerUniverse

THEOREM AdequateLeaderDurablyRetiredOwnerPersistsAcrossCorridorStep ==
  \A target, leaderContext, leader, leaderView, owner:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateServiceTombstoneLifecycleInvariant
    /\ AdequateLeaderFrozenTargetCorridor(
         target, leaderContext, leader, leaderView)
    /\ owner \in
         AdequateLeaderTargetDurablyRetiredOwnerIdentitySet(
           target, leaderContext, leader, leaderView)
    /\ [AsyncNext]_AsyncAllVars
    /\ AdequateLeaderFrozenTargetCorridor(
         target, leaderContext, leader, leaderView)'
    => owner \in
         AdequateLeaderTargetDurablyRetiredOwnerIdentitySet(
           target, leaderContext, leader, leaderView)'
BY AdequateLeaderInternalBodyAvailableRetiredOwnerSetIsMonotoneAtGst,
   AdequateLeaderProducerContinuationRetiredOwnerSetIsMonotoneAtGst,
   AdequateLeaderOffSubjectControlRetirementMemoryIsStepInvariant,
   AdequateLeaderLiveAndServicedCandidateIdentitiesAreDisjoint,
   AdequateLeaderServicedCandidateIdentityHasServiceWitness,
   AdequateLeaderServicedCandidateMemoryAndClosureAreStepInvariant,
   AdequateLeaderAsyncBracketStepPreservesTargetDecision,
   AdequateLeaderFrozenSubjectSwitchOwnerUniverseIsPrimeInvariant,
   IsaT(900)
   DEF AdequateLeaderTargetDurablyRetiredOwnerIdentitySet,
       AdequateLeaderTargetServicedCandidateOwnerIdentitySet,
       AdequateLeaderTargetProducerContinuationRetiredOwnerIdentitySet,
       AdequateLeaderTargetDecisionRetiredCandidateOwnerIdentitySet,
       AdequateLeaderCandidateProducerContinuationRetirementMemory,
       AdequateLeaderServicedCandidateMemory,
       AdequateLeaderServicedCandidateClosure,
       AdequateLeaderTargetOffSubjectControlClosedOwnerIdentitySet,
       AdequateLeaderTargetOffSubjectControlRetirementClosed,
       AdequateLeaderTargetOffSubjectControlRetirementMemory,
       AdequateLeaderTargetInternalBodyAvailableRetiredOwnerIdentitySet,
       AdequateLeaderFrozenTargetCorridor,
       AdequateLeaderFrozenSubjectSwitchOwnerUniverse,
       AsyncAllVars

AdequateLeaderTargetDurableRetirementCarryProperty(specification) ==
  specification
    => [](\A target \in ValidatorIds,
             leaderContext \in ContextRecords,
             leader \in ValidatorIds,
             leaderView \in Views,
             owner \in
               AdequateLeaderFrozenSubjectSwitchOwnerUniverse(
                 target, leaderContext, leader, leaderView):
            /\ AdequateLeaderFrozenTargetCorridor(
                 target, leaderContext, leader, leaderView)
            /\ owner \in
                 AdequateLeaderTargetDurablyRetiredOwnerIdentitySet(
                   target, leaderContext, leader, leaderView)
              => [](AdequateLeaderFrozenTargetCorridor(
                       target, leaderContext, leader, leaderView)
                    => owner \in
                         AdequateLeaderTargetDurablyRetiredOwnerIdentitySet(
                           target, leaderContext, leader, leaderView)))

THEOREM AsyncLiveProvidesAdequateLeaderTargetDurableRetirementCarry ==
  \A initialContext:
    AdequateLeaderTargetDurableRetirementCarryProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysProgressOwnershipInvariant,
   AsyncSpecAlwaysCandidateServiceTombstoneLifecycle,
   AsyncLiveSpecProjectsAsyncSpec,
   AdequateLeaderDurablyRetiredOwnerPersistsAcrossCorridorStep,
   PTL
   DEF AdequateLeaderTargetDurableRetirementCarryProperty,
       AsyncAllVars

AdequateLeaderTargetDurableRetirementSnapshotCarryProperty(specification) ==
  specification
    => [](\A target \in ValidatorIds,
             leaderContext \in ContextRecords,
             leader \in ValidatorIds,
             leaderView \in Views,
             retired \in
               SUBSET AdequateLeaderFrozenSubjectSwitchOwnerUniverse(
                 target, leaderContext, leader, leaderView):
            /\ AdequateLeaderFrozenTargetCorridor(
                 target, leaderContext, leader, leaderView)
            /\ retired \subseteq
                 AdequateLeaderTargetDurablyRetiredOwnerIdentitySet(
                   target, leaderContext, leader, leaderView)
              => [](AdequateLeaderFrozenTargetCorridor(
                       target, leaderContext, leader, leaderView)
                    => retired \subseteq
                         AdequateLeaderTargetDurablyRetiredOwnerIdentitySet(
                           target, leaderContext, leader, leaderView)))

THEOREM AdequateLeaderDurableRetirementCarryLiftsToFrozenSnapshots ==
  \A initialContext:
    AdequateLeaderTargetDurableRetirementCarryProperty(
      AsyncLiveSpecAt(initialContext))
      => AdequateLeaderTargetDurableRetirementSnapshotCarryProperty(
           AsyncLiveSpecAt(initialContext))
BY PTL, Isa
   DEF AdequateLeaderTargetDurableRetirementCarryProperty,
       AdequateLeaderTargetDurableRetirementSnapshotCarryProperty

THEOREM AdequateLeaderTargetSubjectSwitchBudgetIsFinite ==
  \A target, leaderContext, leader, leaderView, retired:
    /\ target \in ValidatorIds
    /\ leaderContext \in ContextRecords
    /\ leader \in ValidatorIds
    /\ leaderView \in Nat
    /\ AdequateLeaderTargetSubjectSwitchRetiredOwnerSet(
         target, leaderContext, leader, leaderView, retired)
    => /\ AdequateLeaderTargetSubjectSwitchRemainingBudget(
             target, leaderContext, leader, leaderView, retired) \in Nat
       /\ AdequateLeaderTargetSubjectSwitchRemainingBudget(
             target, leaderContext, leader, leaderView, retired)
            <= Cardinality(
                 AdequateLeaderFrozenSubjectSwitchOwnerUniverse(
                   target, leaderContext, leader, leaderView))
BY AdequateLeaderFrozenSubjectSwitchOwnerUniverseIsFinite,
   FS_Subset, FS_CardinalityType, Isa
   DEF AdequateLeaderTargetSubjectSwitchRetiredOwnerSet,
       AdequateLeaderTargetDurablyRetiredOwnerIdentitySet,
       AdequateLeaderTargetSubjectSwitchRemainingBudget

AdequateLeaderTargetOpenFrontier(
    target, leaderContext, leader, leaderView, subject) ==
  \/ AdequateLeaderTargetProducerTransportResidual(
       target, leaderContext, leader, leaderView, subject)
  \/ \E occurrenceRank \in AdequateLeaderTargetOccurrenceRankCarrier:
       AdequateLeaderTargetOccurrenceRankFrontier(
         target, leaderContext, leader, leaderView, subject, occurrenceRank)

AdequateLeaderTargetProductiveSubjectOpenFrontier(
    target, leaderContext, leader, leaderView, subject) ==
  /\ AdequateLeaderTargetProtocolSubjectSource(
       target, leaderContext, leader, leaderView, subject)
  /\ AdequateLeaderTargetOpenFrontier(
       target, leaderContext, leader, leaderView, subject)

(***************************************************************************
Exact fixed-corridor exit handoff.

Leaving one target/context/leader/view corridor is not Decision and is not a
decrease of the occurrence rank.  The handoff therefore retains the exact
selected owner and one candidate whose immutable payload proves that owner's
target/context/leader/view/subject/phase lineage.  A transport retry has the
same service identity; a serviced or obsolete occurrence is represented by
the existing candidate tombstone/lifecycle record or by the installed TC
which retired its consumer.  The separate outer view-reach property is the
only temporal consumer of this predicate.
***************************************************************************)
AdequateLeaderTargetOccurrenceCandidateExitLineage(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank, owner, candidate) ==
  /\ sourceOccurrenceRank \in
       AdequateLeaderTargetOccurrenceRankCarrier
  /\ AdequateLeaderFrozenTargetCandidateIdentity(
       candidate, sourceOccurrenceRank[1],
       target, leaderContext, leader, leaderView, subject)
  /\ owner =
       AdequateLeaderFrozenCandidateOwnerIdentity(
         candidate, sourceOccurrenceRank[1],
         target, leaderContext, leader, leaderView, subject)
  /\ \/ ResponsiveProtectedCandidateOwned(candidate)
     \/ NodeHasDecision(candidate.node)
     \/ NodeHasApplication(candidate.node)
     \/ ExactLeaderCandidatePostMilestone(
          candidate, sourceOccurrenceRank[1])
     \/ SameIdentityLeaderOwner(candidate)
     \/ ExactLeaderEvidenceAt(candidate, sourceOccurrenceRank[1])
     \/ ExactLeaderDiscardProvenanceAt(
          candidate, sourceOccurrenceRank[1])
     \/ AsyncCandidateServiceTombstoned(candidate)
     \/ AsyncCandidateLifecycleRecorded(
          candidate.node, candidate.causalOrigin)
     \/ /\ ~CandidateConsumerCurrent(candidate)
        /\ \/ candidate.consumerContext # context
           \/ \E installed \in installedTCs:
                /\ installed.node = candidate.node
                /\ installed.tc.context = candidate.consumerContext
                /\ installed.tc.view >= candidate.consumerView

AdequateLeaderTargetOccurrenceCorridorExitHandoff(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank, owner) ==
  LET authority ==
        AdequateLeaderCorridorAuthorityReceipt(
          target, leaderContext, leader, leaderView)
  IN /\ gst
  /\ AdequateLeaderCorridorAuthorityReceiptValid(authority)
  /\ authority.context = context
  /\ owner.authority = authority
  /\ ~AdequateLeaderFrozenTargetCorridor(
       target, leaderContext, leader, leaderView)
  /\ owner \in
       AdequateLeaderFrozenCandidateOwnerUniverse(
         target, leaderContext, leader, leaderView, subject)
  /\ \E candidate \in AsyncCandidateSet:
       AdequateLeaderTargetOccurrenceCandidateExitLineage(
         target, leaderContext, leader, leaderView,
         subject, sourceOccurrenceRank, owner, candidate)

AdequateLeaderTargetAnyCorridorExitHandoff(
    target, leaderContext, leader, leaderView) ==
  \E subject \in Subjects,
     sourceOccurrenceRank \in
       AdequateLeaderTargetOccurrenceRankCarrier,
     owner \in
       AdequateLeaderFrozenCandidateOwnerUniverse(
         target, leaderContext, leader, leaderView, subject):
    AdequateLeaderTargetOccurrenceCorridorExitHandoff(
      target, leaderContext, leader, leaderView,
      subject, sourceOccurrenceRank, owner)

THEOREM AdequateLeaderSelectedOwnerPhysicalCorridorTerminalIsExactHandoff ==
  \A target, leaderContext, leader, leaderView,
     subject, sourceOccurrenceRank, owner, sourceCandidates:
    /\ owner \in
         AdequateLeaderFrozenCandidateOwnerUniverse(
           target, leaderContext, leader, leaderView, subject)
    /\ AdequateLeaderTargetSelectedOwnerPhysicalEpisodeCorridorTerminal(
         target, leaderContext, leader, leaderView,
         subject, sourceOccurrenceRank, owner, sourceCandidates)
    => AdequateLeaderTargetOccurrenceCorridorExitHandoff(
         target, leaderContext, leader, leaderView,
         subject, sourceOccurrenceRank, owner)
BY Isa
   DEF AdequateLeaderTargetSelectedOwnerPhysicalEpisodeCorridorTerminal,
       AdequateLeaderTargetOccurrenceCorridorExitHandoff,
       AdequateLeaderTargetOccurrenceCandidateExitLineage

AdequateLeaderTargetFrozenCorridorTerminalGoal(
    target, leaderContext, leader, leaderView) ==
  \/ NodeHasDecision(target)
  \/ AdequateLeaderTargetAnyCorridorExitHandoff(
       target, leaderContext, leader, leaderView)

\* This state-only predicate is diagnostic, not a release endpoint.  It says
\* that some productive subject is open, but does not retain the retiring
\* owner or the accumulated global owner budget.  The release carry below
\* therefore binds `retired2`, `budget2`, and the next selected owner instead
\* of terminating at this predicate.  Corridor exit remains separate for the
\* rotating-leader/view kernel; it is not relabelled as fixed-view Decision
\* progress.
AdequateLeaderTargetProductiveSubjectReentryGoal(
    target, leaderContext, leader, leaderView, retiredSubject) ==
  /\ retiredSubject \in Subjects
  /\ \/ NodeHasDecision(target)
     \/ AdequateLeaderTargetAnyCorridorExitHandoff(
          target, leaderContext, leader, leaderView)
     \/ \E nextSubject \in Subjects:
          AdequateLeaderTargetProductiveSubjectOpenFrontier(
            target, leaderContext, leader, leaderView, nextSubject)

AdequateLeaderTargetCarriedOwnerEpisodeAtBudget(
    target, leaderContext, leader, leaderView,
    subject, occurrenceRank, owner, retired, budget) ==
  /\ AdequateLeaderTargetSubjectSwitchRetiredOwnerSet(
       target, leaderContext, leader, leaderView, retired)
  /\ budget =
       AdequateLeaderTargetSubjectSwitchRemainingBudget(
         target, leaderContext, leader, leaderView, retired)
  /\ AdequateLeaderFrozenTargetCorridor(
       target, leaderContext, leader, leaderView)
  /\ subject \in Subjects
  /\ AdequateLeaderTargetOccurrenceOwnerSelected(
       target, leaderContext, leader, leaderView,
       subject, occurrenceRank, owner)
  /\ owner \in
       AdequateLeaderTargetLiveCandidateOwnerIdentitySet(
         target, leaderContext, leader, leaderView, subject)
  /\ owner \in
       AdequateLeaderFrozenSubjectSwitchOwnerUniverse(
         target, leaderContext, leader, leaderView)
       \ retired
  /\ ~AdequateLeaderTargetStrictOccurrenceDescentGoal(
       target, leaderContext, leader, leaderView,
       subject, occurrenceRank)

AdequateLeaderTargetSubjectSwitchEpisodeAtBudget(
    target, leaderContext, leader, leaderView,
    subject, occurrenceRank, owner, retired, budget) ==
  /\ AdequateLeaderTargetCarriedOwnerEpisodeAtBudget(
       target, leaderContext, leader, leaderView,
       subject, occurrenceRank, owner, retired, budget)
  /\ subject # AsyncProposalSubject(leader)

AdequateLeaderTargetProductiveOwnerEpisodeAtBudget(
    target, leaderContext, leader, leaderView,
    subject, occurrenceRank, owner, retired, budget) ==
  /\ AdequateLeaderTargetCarriedOwnerEpisodeAtBudget(
       target, leaderContext, leader, leaderView,
       subject, occurrenceRank, owner, retired, budget)
  /\ AdequateLeaderTargetProtocolSubjectSource(
       target, leaderContext, leader, leaderView, subject)

\* `retired2` is the exact current durable projection, while `retired` is the
\* immutable source snapshot.  Requiring the named owner to be newly included
\* in `retired2` is the completion/tombstone witness.  Its frozen identity
\* already contains subject, phase, semantic rank, target, leader, and view, so
\* no unrelated old closure can discharge this endpoint.
AdequateLeaderTargetOffSubjectRetirementAndReentryGoal(
    target, leaderContext, leader, leaderView,
    subject, occurrenceRank, owner, retired, budget) ==
  \/ NodeHasDecision(target)
  \/ AdequateLeaderTargetOccurrenceCorridorExitHandoff(
       target, leaderContext, leader, leaderView,
       subject, occurrenceRank, owner)
  \/ /\ owner \notin retired
     /\ \E retired2 \in
              SUBSET AdequateLeaderFrozenSubjectSwitchOwnerUniverse(
                target, leaderContext, leader, leaderView),
            budget2 \in
              SetLessThan(budget, OpToRel(<, Nat), Nat),
            nextSubject \in Subjects,
            nextOccurrenceRank \in
              AdequateLeaderTargetOccurrenceRankCarrier,
            nextOwner \in
              AdequateLeaderFrozenCandidateOwnerUniverse(
                target, leaderContext, leader, leaderView, nextSubject):
          /\ retired \cup {owner} \subseteq retired2
          /\ owner \in retired2
          /\ budget2 =
               AdequateLeaderTargetSubjectSwitchRemainingBudget(
                 target, leaderContext, leader, leaderView, retired2)
          /\ AdequateLeaderTargetProductiveOwnerEpisodeAtBudget(
               target, leaderContext, leader, leaderView,
               nextSubject, nextOccurrenceRank, nextOwner,
               retired2, budget2)

THEOREM AdequateLeaderOffSubjectReentryCarriesRetiredOwnerAndBudget ==
  \A target, leaderContext, leader, leaderView,
     subject, occurrenceRank, owner, retired, budget:
    /\ ~NodeHasDecision(target)
    /\ AdequateLeaderFrozenTargetCorridor(
         target, leaderContext, leader, leaderView)
    /\ AdequateLeaderTargetOffSubjectRetirementAndReentryGoal(
         target, leaderContext, leader, leaderView,
         subject, occurrenceRank, owner, retired, budget)
    => \E retired2 \in
             SUBSET AdequateLeaderFrozenSubjectSwitchOwnerUniverse(
               target, leaderContext, leader, leaderView),
           budget2 \in
             SetLessThan(budget, OpToRel(<, Nat), Nat),
           nextSubject \in Subjects,
           nextOccurrenceRank \in
             AdequateLeaderTargetOccurrenceRankCarrier,
           nextOwner \in
             AdequateLeaderFrozenCandidateOwnerUniverse(
               target, leaderContext, leader, leaderView, nextSubject):
         /\ retired \cup {owner} \subseteq retired2
         /\ owner \in retired2
         /\ budget2 =
              AdequateLeaderTargetSubjectSwitchRemainingBudget(
                target, leaderContext, leader, leaderView, retired2)
         /\ AdequateLeaderTargetProductiveOwnerEpisodeAtBudget(
              target, leaderContext, leader, leaderView,
              nextSubject, nextOccurrenceRank, nextOwner,
              retired2, budget2)
BY Isa
   DEF AdequateLeaderTargetOffSubjectRetirementAndReentryGoal

AdequateLeaderTargetSubjectSwitchDiscoveredOwnerSet(owner, retired) ==
  {owner} \ retired

AdequateLeaderTargetSubjectSwitchEpisodeAdvanceGoal(
    target, leaderContext, leader, leaderView,
    subject, occurrenceRank, owner, retired, budget) ==
  \E discovered,
     retired2 \in
       SUBSET AdequateLeaderFrozenSubjectSwitchOwnerUniverse(
         target, leaderContext, leader, leaderView),
     budget2 \in Nat,
     nextSubject \in Subjects,
     nextOccurrenceRank \in
       AdequateLeaderTargetOccurrenceRankCarrier,
     nextOwner \in
       AdequateLeaderFrozenSubjectSwitchOwnerUniverse(
         target, leaderContext, leader, leaderView):
    /\ discovered =
         AdequateLeaderTargetSubjectSwitchDiscoveredOwnerSet(
           owner, retired)
    /\ discovered # {}
    /\ retired \cup discovered \subseteq retired2
    /\ owner \in retired2
    /\ budget2 =
         AdequateLeaderTargetSubjectSwitchRemainingBudget(
           target, leaderContext, leader, leaderView, retired2)
    /\ budget2 < budget
    /\ AdequateLeaderTargetSubjectSwitchEpisodeAtBudget(
         target, leaderContext, leader, leaderView,
         nextSubject, nextOccurrenceRank,
         nextOwner, retired2, budget2)

AdequateLeaderTargetSubjectSwitchBudgetDescentGoal(
    target, leaderContext, leader, leaderView,
    subject, occurrenceRank, owner, retired, budget) ==
  \/ AdequateLeaderTargetOffSubjectRetirementAndReentryGoal(
       target, leaderContext, leader, leaderView,
       subject, occurrenceRank, owner, retired, budget)
  \/ AdequateLeaderTargetSubjectSwitchEpisodeAdvanceGoal(
       target, leaderContext, leader, leaderView,
       subject, occurrenceRank, owner, retired, budget)

THEOREM AdequateLeaderFrozenCorridorHasProductiveSubjectReentry ==
  \A target, leaderContext, leader, leaderView, retiredSubject:
    /\ AsyncStrongTypeInvariant
    /\ retiredSubject \in Subjects
    /\ AdequateLeaderFrozenTargetCorridor(
         target, leaderContext, leader, leaderView)
    => AdequateLeaderTargetProductiveSubjectReentryGoal(
         target, leaderContext, leader, leaderView, retiredSubject)
BY IsaT(600)
   DEF AdequateLeaderTargetProductiveSubjectReentryGoal,
       AdequateLeaderTargetProductiveSubjectOpenFrontier,
       AdequateLeaderTargetOpenFrontier,
       AdequateLeaderTargetProtocolSubjectSource,
       AdequateLeaderTargetProducerTransportResidual,
       AdequateLeaderTargetProducerResidual,
       AdequateLeaderTargetCommitQcRebroadcastResidual,
       AdequateLeaderTargetDueTransportResidual,
       AdequateLeaderTargetRunnerAdmissionResidual,
       AdequateLeaderTargetCertifiedResponseCapacityResidual,
       AdequateLeaderTargetRankFrontier,
       AsyncStrongTypeInvariant

THEOREM AdequateLeaderSubjectSwitchNamedOwnerStrictlyConsumesBudget ==
  \A target, leaderContext, leader, leaderView,
     owner, retired, retired2, budget:
    /\ target \in ValidatorIds
    /\ leaderContext \in ContextRecords
    /\ leader \in ValidatorIds
    /\ leaderView \in Nat
    /\ retired \subseteq
         AdequateLeaderFrozenSubjectSwitchOwnerUniverse(
           target, leaderContext, leader, leaderView)
    /\ retired2 \subseteq
         AdequateLeaderFrozenSubjectSwitchOwnerUniverse(
           target, leaderContext, leader, leaderView)
    /\ owner \in
         AdequateLeaderFrozenSubjectSwitchOwnerUniverse(
           target, leaderContext, leader, leaderView)
         \ retired
    /\ retired \cup {owner} \subseteq retired2
    /\ budget =
         AdequateLeaderTargetSubjectSwitchRemainingBudget(
           target, leaderContext, leader, leaderView, retired)
    => /\ AdequateLeaderTargetSubjectSwitchDiscoveredOwnerSet(
             owner, retired) = {owner}
       /\ AdequateLeaderTargetSubjectSwitchRemainingBudget(
            target, leaderContext, leader, leaderView, retired2)
            < budget
BY AdequateLeaderFrozenSubjectSwitchOwnerUniverseIsFinite,
   FS_Union, FS_Subset, FS_CardinalityType, IsaT(300)
   DEF AdequateLeaderTargetSubjectSwitchDiscoveredOwnerSet,
       AdequateLeaderTargetSubjectSwitchRemainingBudget,
       AdequateLeaderFrozenSubjectSwitchOwnerUniverse,
       AdequateLeaderFrozenOwnerUniverse

THEOREM AdequateLeaderSubjectSwitchEpisodeStartsNamedOwnerService ==
  \A target, leaderContext, leader, leaderView,
     subject, occurrenceRank, owner, retired, budget:
    /\ AsyncStrongTypeInvariant
    /\ AdequateLeaderTargetSubjectSwitchEpisodeAtBudget(
         target, leaderContext, leader, leaderView,
         subject, occurrenceRank, owner, retired, budget)
    => \E known \in
         SUBSET AdequateLeaderFrozenOwnerUniverse(
           target, leaderContext, leader, leaderView, subject),
         serviceBudget \in Nat:
         /\ known =
              AdequateLeaderTargetLiveOwnerIdentitySet(
                target, leaderContext, leader, leaderView, subject)
         /\ AdequateLeaderTargetNonDescentEpisodeAtBudget(
              target, leaderContext, leader, leaderView,
              subject, occurrenceRank, known, serviceBudget)
         /\ AdequateLeaderTargetSameOrHigherOccurrenceFrontier(
              target, leaderContext, leader, leaderView,
              subject, occurrenceRank)
         /\ AdequateLeaderTargetOccurrenceOwnerSelected(
              target, leaderContext, leader, leaderView,
              subject, occurrenceRank, owner)
BY AdequateLeaderTargetCurrentOwnersInitializeKnownEpisode,
   AdequateLeaderTargetNonDescentEpisodeBudgetIsFiniteAndCoalesced,
   IsaT(300)
   DEF AdequateLeaderTargetSubjectSwitchEpisodeAtBudget,
       AdequateLeaderTargetCarriedOwnerEpisodeAtBudget,
       AdequateLeaderTargetNonDescentEpisodeAtBudget,
       AdequateLeaderTargetNonDescentEpisodeFrontier,
       AdequateLeaderTargetOccurrenceEpisodeActive,
       AdequateLeaderTargetSameOrHigherOccurrenceFrontier,
       AdequateLeaderTargetEpisodeStartsWithCurrentOwners,
       AdequateLeaderTargetEpisodeKnownOwnerSet,
       AdequateLeaderTargetNonDescentEpisodeBudget

AdequateLeaderViewReachCompositionProperty(specification) ==
  specification
    => /\ \A target \in ValidatorIds:
             AdequateLeaderLocalTargetDecisionSource(target)
               ~> (NodeHasDecision(target)
                    \/ AdequateLeaderTargetDecisionSource(target))
       /\ \A target \in ValidatorIds,
             leaderContext \in ContextRecords,
             leader \in ValidatorIds,
             leaderView \in Views:
             AdequateLeaderTargetAnyCorridorExitHandoff(
               target, leaderContext, leader, leaderView)
               ~> NodeHasDecision(target)

AdequateLeaderTargetCorridorEntryProperty(specification) ==
  specification
    => \A target \in ValidatorIds:
         AdequateLeaderTargetDecisionSource(target)
           ~> (NodeHasDecision(target)
                \/ \E leaderContext \in ContextRecords,
                      leader \in ValidatorIds,
                      leaderView \in Views,
                      subject \in Subjects:
                     AdequateLeaderTargetProductiveSubjectOpenFrontier(
                       target, leaderContext, leader, leaderView, subject))

AdequateLeaderTargetProducerTransportClosureProperty(specification) ==
  specification
    => \A target \in ValidatorIds,
          leaderContext \in ContextRecords,
          leader \in ValidatorIds,
          leaderView \in Views,
          subject \in Subjects:
         /\ AdequateLeaderTargetProtocolSubjectSource(
              target, leaderContext, leader, leaderView, subject)
         /\ AdequateLeaderTargetProducerTransportResidual(
              target, leaderContext, leader, leaderView, subject)
           ~> (NodeHasDecision(target)
                \/ \E nextSubject \in Subjects,
                      occurrenceRank \in
                        AdequateLeaderTargetOccurrenceRankCarrier:
                     /\ AdequateLeaderTargetProtocolSubjectSource(
                          target, leaderContext, leader,
                          leaderView, nextSubject)
                     /\ AdequateLeaderTargetOccurrenceRankFrontier(
                          target, leaderContext, leader,
                          leaderView, nextSubject, occurrenceRank))

\* The target-subject arm is the only arm allowed to enter the producer
\* corridor or to expose a strict occurrence decrease.  A foreign subject
\* may only drain its exact named owner to durable retirement; the separate
\* finite subject-switch episode then consumes that retirement.  Neither a
\* selector change nor foreign-subject drain is protocol progress.
AdequateLeaderTargetProductiveOccurrenceServiceGoal(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank, known) ==
  /\ AdequateLeaderTargetProtocolSubjectSource(
       target, leaderContext, leader, leaderView, subject)
  /\ \/ AdequateLeaderTargetNonDescentEpisodeResidual(
          target, leaderContext, leader,
          leaderView, subject, sourceOccurrenceRank, known)
     \/ /\ AdequateLeaderTargetProducerTransportResidualAtOccurrence(
               target, leaderContext, leader,
               leaderView, subject, sourceOccurrenceRank)
        /\ AdequateLeaderTargetLiveOwnerIdentitySet(
             target, leaderContext, leader, leaderView, subject)
             \subseteq known

AdequateLeaderTargetOffSubjectOccurrenceDrainGoal(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank, owner) ==
  /\ ~AdequateLeaderTargetProtocolSubjectSource(
       target, leaderContext, leader, leaderView, subject)
  /\ AdequateLeaderTargetOccurrenceOwnerRetirementClosed(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank, owner)

\* This is the release bookkeeping carried between same-rank discovery,
\* producer/transport work, and subject switches.  The owner must have been
\* named by the episode: it is either still the exact selected live owner or
\* its own durable retirement is visible.  Membership in `known` alone is not
\* enough, because that would re-admit an unrelated old tombstone.
AdequateLeaderTargetOccurrenceOwnerCarried(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank, known, owner) ==
  /\ owner \in known
  /\ owner \in
       AdequateLeaderFrozenCandidateOwnerUniverse(
         target, leaderContext, leader, leaderView, subject)
  /\ \/ AdequateLeaderTargetOccurrenceOwnerSelected(
          target, leaderContext, leader, leaderView,
          subject, sourceOccurrenceRank, owner)
     \/ AdequateLeaderTargetOccurrenceOwnerRetirementClosed(
          target, leaderContext, leader, leaderView,
          subject, sourceOccurrenceRank, owner)

AdequateLeaderTargetOccurrenceDecisionGoal(target) ==
  NodeHasDecision(target)

AdequateLeaderTargetOccurrenceStrictlyLowerGoal(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank) ==
  /\ AdequateLeaderTargetProtocolSubjectSource(
       target, leaderContext, leader, leaderView, subject)
  /\ \E lowerOccurrenceRank \in
       SetLessThan(
         sourceOccurrenceRank,
         AdequateLeaderTargetOccurrenceRankOrdering,
         AdequateLeaderTargetOccurrenceRankCarrier):
       AdequateLeaderTargetOccurrenceRankFrontier(
         target, leaderContext, leader,
         leaderView, subject, lowerOccurrenceRank)

\* Equal-count owner replacement and count-increasing replenishment enter
\* this finite episode arm.  An off-subject owner may leave the active
\* selector only after that exact owner has durable retirement memory.  None
\* of these alternatives is called Decision or occurrence-rank descent.
AdequateLeaderTargetOccurrenceEqualOwnerOrProducerEpisodeGoal(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank, known, owner) ==
  \/ /\ AdequateLeaderTargetProductiveOccurrenceServiceGoal(
          target, leaderContext, leader,
          leaderView, subject, sourceOccurrenceRank, known)
     /\ AdequateLeaderTargetOccurrenceOwnerCarried(
          target, leaderContext, leader, leaderView,
          subject, sourceOccurrenceRank, known, owner)
  \/ AdequateLeaderTargetOffSubjectOccurrenceDrainGoal(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank, owner)

AdequateLeaderTargetUniversalOccurrenceServiceGoal(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank, known, owner) ==
  \/ AdequateLeaderTargetOccurrenceDecisionGoal(target)
  \/ AdequateLeaderTargetOccurrenceStrictlyLowerGoal(
       target, leaderContext, leader,
       leaderView, subject, sourceOccurrenceRank)
  \/ AdequateLeaderTargetOccurrenceEqualOwnerOrProducerEpisodeGoal(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank, known, owner)
  \/ AdequateLeaderTargetOccurrenceCorridorExitHandoff(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank, owner)

(***************************************************************************
Open semantic handoff after exact physical identity drain.

The finite episode above proves only that every concrete carrier of the
selected immutable candidate owner drains (or the frozen corridor exits).
It deliberately does not turn the complement-style producer residual into
progress.  A concrete producer-origin provider must discharge the temporal
property below by exposing Decision, strict occurrence descent, a genuine
new finite-universe owner, exact off-subject retirement, or the separately
indexed producer/transport corridor.  The retired selected owner remains
bookkeeping; it is never asserted to be the new producer owner.
***************************************************************************)
AdequateLeaderTargetSelectedOwnerSemanticHandoffDebt(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank, known, owner, sourceCandidates) ==
  /\ AdequateLeaderTargetEpisodeKnownOwnerSet(
       target, leaderContext, leader, leaderView, subject, known)
  /\ owner \in known
  /\ AdequateLeaderTargetSelectedOwnerNonContinuationTerminal(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank, owner, sourceCandidates)
  /\ ~AdequateLeaderTargetUniversalOccurrenceServiceGoal(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank, known, owner)

AdequateLeaderTargetSelectedOwnerSemanticHandoffProperty(specification) ==
  specification
    => \A target \in ValidatorIds,
          leaderContext \in ContextRecords,
          leader \in ValidatorIds,
          leaderView \in Views,
          subject \in Subjects,
          sourceOccurrenceRank \in
            AdequateLeaderTargetOccurrenceRankCarrier,
          known \in
            SUBSET AdequateLeaderFrozenOwnerUniverse(
              target, leaderContext, leader, leaderView, subject),
          owner \in
            AdequateLeaderFrozenCandidateOwnerUniverse(
              target, leaderContext, leader, leaderView, subject),
          sourceCandidates \in SUBSET AsyncCandidateSet:
         AdequateLeaderTargetSelectedOwnerSemanticHandoffDebt(
           target, leaderContext, leader, leaderView,
           subject, sourceOccurrenceRank, known, owner, sourceCandidates)
           ~> AdequateLeaderTargetUniversalOccurrenceServiceGoal(
                target, leaderContext, leader, leaderView,
                subject, sourceOccurrenceRank, known, owner)

AdequateLeaderTargetOccurrenceRankOwnerServiceExitGoal(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank, owner) ==
  \/ AdequateLeaderTargetOccurrenceDecisionGoal(target)
  \/ AdequateLeaderTargetOccurrenceStrictlyLowerGoal(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank)
  \/ AdequateLeaderTargetOccurrenceCorridorExitHandoff(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank, owner)
  \/ AdequateLeaderTargetOffSubjectOccurrenceDrainGoal(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank, owner)

AdequateLeaderTargetOccurrenceRankServiceExitGoal(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank, owner) ==
  AdequateLeaderTargetOccurrenceRankOwnerServiceExitGoal(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank, owner)

AdequateLeaderTargetCarriedNonDescentKnownAdvanceGoal(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank, known, budget, owner) ==
  \E discovered,
     known2 \in
       SUBSET AdequateLeaderFrozenOwnerUniverse(
         target, leaderContext, leader, leaderView, subject),
     budget2 \in Nat:
    /\ discovered =
         AdequateLeaderTargetNonDescentDiscoveredOwnerIdentitySet(
           target, leaderContext, leader, leaderView, subject, known)
    /\ discovered # {}
    /\ known2 = known \cup discovered
    /\ AdequateLeaderTargetNonDescentEpisodeAtBudget(
         target, leaderContext, leader, leaderView,
         subject, sourceOccurrenceRank, known2, budget2)
    /\ AdequateLeaderTargetOccurrenceOwnerCarried(
         target, leaderContext, leader, leaderView,
         subject, sourceOccurrenceRank, known2, owner)
    /\ budget2 < budget

AdequateLeaderTargetCarriedNonDescentEpisodeResidual(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank, known, owner) ==
  /\ AdequateLeaderTargetNonDescentEpisodeResidual(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank, known)
  /\ AdequateLeaderTargetOccurrenceOwnerCarried(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank, known, owner)

THEOREM AdequateLeaderCarriedNonDescentResidualAdvancesKnownBudget ==
  \A target, leaderContext, leader, leaderView,
     subject, sourceOccurrenceRank, known, budget, owner:
    /\ AsyncStrongTypeInvariant
    /\ AdequateLeaderTargetCarriedNonDescentEpisodeResidual(
         target, leaderContext, leader, leaderView,
         subject, sourceOccurrenceRank, known, owner)
    /\ budget =
         AdequateLeaderTargetNonDescentEpisodeBudget(
           target, leaderContext, leader, leaderView, subject, known)
    => AdequateLeaderTargetCarriedNonDescentKnownAdvanceGoal(
         target, leaderContext, leader, leaderView,
         subject, sourceOccurrenceRank, known, budget, owner)
BY AdequateLeaderTargetNonDescentDiscoveryStrictlyConsumesBudget,
   FS_Union, IsaT(180)
   DEF AdequateLeaderTargetCarriedNonDescentEpisodeResidual,
       AdequateLeaderTargetCarriedNonDescentKnownAdvanceGoal,
       AdequateLeaderTargetOccurrenceOwnerCarried,
       AdequateLeaderTargetNonDescentKnownAdvanceGoal

\* Once service of a ranked owner opens producer/transport work, the source
\* occurrence remains frozen.  While that subject remains the deterministic
\* proposal subject, the corridor may expose a genuinely new owner relative
\* to the source owner set, but its only terminal rank is strict descent.  If
\* the selector changes first, the old occurrence may terminate only at the
\* separate off-subject same-owner retirement exit; re-entry is handled by the outer
\* subject-switch episode and is not transport progress.
AdequateLeaderTargetProducerTransportOccurrenceClosureProperty(
    specification) ==
  specification
    => \A target \in ValidatorIds,
          leaderContext \in ContextRecords,
          leader \in ValidatorIds,
          leaderView \in Views,
          subject \in Subjects,
          sourceOccurrenceRank \in
            AdequateLeaderTargetOccurrenceRankCarrier,
          known \in
            SUBSET AdequateLeaderFrozenOwnerUniverse(
              target, leaderContext, leader, leaderView, subject),
          budget \in Nat,
          owner \in
            AdequateLeaderFrozenCandidateOwnerUniverse(
              target, leaderContext, leader, leaderView, subject):
         /\ AdequateLeaderTargetNonDescentEpisodeAtBudget(
              target, leaderContext, leader, leaderView,
              subject, sourceOccurrenceRank, known, budget)
         /\ AdequateLeaderTargetOccurrenceOwnerCarried(
              target, leaderContext, leader, leaderView,
              subject, sourceOccurrenceRank, known, owner)
         /\ AdequateLeaderTargetProtocolSubjectSource(
              target, leaderContext, leader, leaderView, subject)
         /\ AdequateLeaderTargetProducerTransportResidualAtOccurrence(
              target, leaderContext, leader, leaderView,
              subject, sourceOccurrenceRank)
           ~> (AdequateLeaderTargetOccurrenceRankServiceExitGoal(
                 target, leaderContext, leader, leaderView,
                 subject, sourceOccurrenceRank, owner)
                \/ AdequateLeaderTargetCarriedNonDescentEpisodeResidual(
                     target, leaderContext, leader, leaderView,
                     subject, sourceOccurrenceRank, known, owner))

(***************************************************************************
Exact producer-origin receipt bridge.

The negative producer residual does not itself identify a producer.  The
carried owner does: it is either still scheduled, has reached the exact
durable internal BodyAvailable preflight terminal, has left this immutable
authority corridor, or owns a retired lifecycle record with its matching
service receipt.  Keeping these cases explicit prevents an arbitrary closed
candidate from being projected into a producer reservation.
***************************************************************************)
AdequateLeaderTargetProducerOriginReceiptSource(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank, known, budget, owner) ==
  /\ AdequateLeaderTargetNonDescentEpisodeAtBudget(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank, known, budget)
  /\ AdequateLeaderTargetOccurrenceOwnerCarried(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank, known, owner)
  /\ AdequateLeaderTargetProtocolSubjectSource(
       target, leaderContext, leader, leaderView, subject)
  /\ AdequateLeaderTargetProducerTransportResidualAtOccurrence(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank)

AdequateLeaderTargetProducerOriginScheduledWitness(
    target, leaderContext, leader, leaderView, subject, owner) ==
  \E candidate \in
       QueuedCandidates \cup DeferredCandidates
         \cup CausalCandidates \cup TrackedWorkCandidates,
     rank \in AdequateLeaderTargetSemanticRankCarrier:
    /\ AdequateLeaderFrozenTargetCandidateIdentity(
         candidate, rank, target, leaderContext,
         leader, leaderView, subject)
    /\ owner = AdequateLeaderFrozenCandidateOwnerIdentity(
         candidate, rank, target, leaderContext,
         leader, leaderView, subject)

AdequateLeaderTargetProducerOriginDurableBodyTerminal(
    target, leaderContext, leader, leaderView, subject, owner) ==
  \E candidate \in AsyncCandidateSet,
     rank \in AdequateLeaderTargetSemanticRankCarrier:
    /\ AdequateLeaderFrozenTargetCandidateIdentity(
         candidate, rank, target, leaderContext,
         leader, leaderView, subject)
    /\ owner = AdequateLeaderFrozenCandidateOwnerIdentity(
         candidate, rank, target, leaderContext,
         leader, leaderView, subject)
    /\ candidate.kind
         \in {"FetchBody", "RebindRetainedBody", "FetchCertifiedBody"}
    /\ AsyncCandidateInternalBodyAvailableStageRetired(candidate)
    /\ ~CandidateScheduled(candidate)

AdequateLeaderTargetProducerOriginAuthorityExit(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank, owner) ==
  /\ AdequateLeaderTargetOccurrenceRankServiceExitGoal(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank, owner)
  /\ owner.target = target
  /\ owner.context = leaderContext
  /\ owner.authority =
       AdequateLeaderCorridorAuthorityReceipt(
         target, leaderContext, leader, leaderView)

AdequateLeaderTargetProducerOriginReceiptWitness(
    target, leaderContext, leader, leaderView, subject,
    owner, lifecycle, serviced) ==
  /\ lifecycle \in AsyncCandidateLifecycleAdmissions
  /\ lifecycle.retired
  /\ serviced \in AsyncCandidateServiceTombstones
  /\ serviced.node = lifecycle.node
  /\ serviced.identity.payload.causalOrigin = lifecycle.origin
  /\ serviced.context = leaderContext
  /\ serviced.height = leaderContext.height
  /\ serviced.view = leaderView
  /\ serviced.subject = subject
  /\ lifecycle.node \in {target, leader}
  /\ AsyncCandidateLifecycleServiceRecordCoversIn(
       asyncControlServiceState, lifecycle)
  /\ \E candidate \in AsyncCandidateSet,
       rank \in AdequateLeaderTargetSemanticRankCarrier:
       /\ AdequateLeaderFrozenTargetCandidateIdentity(
            candidate, rank, target, leaderContext,
            leader, leaderView, subject)
       /\ serviced.identity = AsyncCandidateServiceIdentity(candidate)
       /\ lifecycle.node = candidate.node
       /\ lifecycle.origin = candidate.causalOrigin
       /\ owner = AdequateLeaderFrozenCandidateOwnerIdentity(
            candidate, rank, target, leaderContext,
            leader, leaderView, subject)

THEOREM AdequateLeaderProducerOriginSourceNamesExactReceiptOrPhysicalTerminal ==
  \A target, leaderContext, leader, leaderView,
     subject, sourceOccurrenceRank, known, budget, owner:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateServiceTombstoneLifecycleInvariant
    /\ AdequateLeaderTargetProducerOriginReceiptSource(
         target, leaderContext, leader, leaderView,
         subject, sourceOccurrenceRank, known, budget, owner)
    => \/ AdequateLeaderTargetProducerOriginScheduledWitness(
             target, leaderContext, leader, leaderView, subject, owner)
       \/ AdequateLeaderTargetProducerOriginDurableBodyTerminal(
             target, leaderContext, leader, leaderView, subject, owner)
       \/ AdequateLeaderTargetProducerOriginAuthorityExit(
             target, leaderContext, leader, leaderView,
             subject, sourceOccurrenceRank, owner)
       \/ \E lifecycle \in AsyncCandidateLifecycleAdmissions,
             serviced \in AsyncCandidateServiceTombstones:
            AdequateLeaderTargetProducerOriginReceiptWitness(
              target, leaderContext, leader, leaderView, subject,
              owner, lifecycle, serviced)
BY AdequateLeaderFrozenCandidateOwnerCarriesAuthorityReceipt,
   AdequateLeaderOwnerIdentityDeterminesNetworkServiceIdentity,
   IsaT(900)
   DEF AdequateLeaderTargetProducerOriginReceiptSource,
       AdequateLeaderTargetProducerOriginScheduledWitness,
       AdequateLeaderTargetProducerOriginDurableBodyTerminal,
       AdequateLeaderTargetProducerOriginAuthorityExit,
       AdequateLeaderTargetProducerOriginReceiptWitness,
       AdequateLeaderTargetOccurrenceOwnerCarried,
       AdequateLeaderTargetOccurrenceOwnerSelected,
       AdequateLeaderTargetOccurrenceOwnerRetirementClosed,
       AdequateLeaderTargetServicedCandidateOwnerIdentitySet,
       AdequateLeaderTargetInternalBodyAvailableRetiredOwnerIdentitySet,
       AdequateLeaderTargetProducerContinuationRetiredOwnerIdentitySet,
       AdequateLeaderCandidateProducerContinuationRetirementMemory,
       AdequateLeaderTargetOffSubjectControlClosedOwnerIdentitySet,
       AdequateLeaderTargetOffSubjectControlOccurrenceIdentity,
       AsyncCandidateProducerContinuationRestartStableTerminalIn,
       AsyncCandidateLifecycleRecordForServiceIn,
       AsyncCandidateLifecycleServiceRecordCoversIn,
       AsyncCandidateServiceTombstoned,
       AsyncCandidateServiceRecordsFor,
       AsyncCandidateServiceRecordsForIdentity,
       AsyncCandidateTransientServiceRecordsForIdentity,
       AsyncCandidateTerminalRecordsForIdentity,
       CandidateScheduled

(***************************************************************************
The receipt branch consumes the semantic-handoff coverage invariant.  Its
continuation case exposes the complete inherited lifecycle token; the other
two cases retain the exact active leader-wire origin or an already frozen
durable replay origin.  A service receipt by itself is deliberately absent
from the conclusion.
***************************************************************************)
THEOREM AdequateLeaderProducerOriginReceiptExposesExactReplacement ==
  \A target, leaderContext, leader, leaderView, subject,
     owner, lifecycle, serviced:
    /\ AsyncCandidateServiceTombstoneLifecycleInvariant
    /\ AdequateLeaderTargetProducerOriginReceiptWitness(
         target, leaderContext, leader, leaderView, subject,
         owner, lifecycle, serviced)
    => \/ \E continuation \in AsyncCandidateProducerContinuations:
             /\ AsyncCandidateProducerSemanticHandoffReservation(
                  continuation)
             /\ (AsyncCandidateProducerSemanticHandoffReservationToken(
                    continuation)).node = lifecycle.node
             /\ (AsyncCandidateProducerSemanticHandoffReservationToken(
                    continuation)).origin = lifecycle.origin
             /\ (AsyncCandidateProducerSemanticHandoffReservationToken(
                    continuation)).slot = lifecycle.slot
             /\ (AsyncCandidateProducerSemanticHandoffReservationToken(
                    continuation)).ordinal = lifecycle.ordinal
       \/ \E wire \in asyncLeaderWireLifecycles:
             /\ wire.recipient = lifecycle.node
             /\ wire.causalOrigin = lifecycle.origin
             /\ AsyncLeaderWireLifecycleActive(wire)
       \/ lifecycle.origin
            \in AsyncCandidateLifecycleDurableReplayOriginsForNode(
                 lifecycle.node)
BY AsyncCandidateLifecycleProducerContinuationCoverageUsesInheritedToken,
   IsaT(300)
   DEF AsyncCandidateServiceTombstoneLifecycleInvariant,
       AsyncCandidateServiceLifecycleInvariant,
       AsyncCandidateProducerSemanticHandoffCoverageInvariant,
       AdequateLeaderTargetProducerOriginReceiptWitness,
       AsyncCandidateLifecycleProducerContinuationCoversIn,
       AsyncLeaderWireLifecycleCoversCandidateRecord

\* One physical service episode freezes exactly the owners present at the
\* source occurrence and names the exact owner which fairness must discharge.
\* Equal-count replacement and count-increasing replenishment remain discovery
\* only.  Universal quantification over `owner` prevents an authenticated
\* foreign-subject candidate from being replaced by an unrelated closed
\* endpoint; its distinct terminal closes that same immutable identity.
AdequateLeaderTargetOccurrenceRankServiceProperty(specification) ==
  specification
    => \A target \in ValidatorIds,
          leaderContext \in ContextRecords,
          leader \in ValidatorIds,
          leaderView \in Views,
          subject \in Subjects,
          sourceOccurrenceRank \in
            AdequateLeaderTargetOccurrenceRankCarrier,
          known \in
            SUBSET AdequateLeaderFrozenOwnerUniverse(
              target, leaderContext, leader, leaderView, subject),
          budget \in Nat,
          owner \in
            AdequateLeaderFrozenCandidateOwnerUniverse(
              target, leaderContext, leader, leaderView, subject):
         /\ AdequateLeaderTargetNonDescentEpisodeAtBudget(
              target, leaderContext, leader, leaderView,
              subject, sourceOccurrenceRank, known, budget)
         /\ AdequateLeaderTargetSameOrHigherOccurrenceFrontier(
              target, leaderContext, leader, leaderView,
              subject, sourceOccurrenceRank)
         /\ AdequateLeaderTargetOccurrenceOwnerSelected(
              target, leaderContext, leader, leaderView,
              subject, sourceOccurrenceRank, owner)
           ~> AdequateLeaderTargetUniversalOccurrenceServiceGoal(
                target, leaderContext, leader, leaderView,
                subject, sourceOccurrenceRank, known, owner)

THEOREM AdequateLeaderPhysicalDrainAndSemanticHandoffProvideOccurrenceRankService ==
  \A initialContext:
    AdequateLeaderTargetSelectedOwnerSemanticHandoffProperty(
      AsyncLiveSpecAt(initialContext))
      => AdequateLeaderTargetOccurrenceRankServiceProperty(
           AsyncLiveSpecAt(initialContext))
BY AsyncLiveProvidesAdequateLeaderSelectedOwnerPhysicalOutcome,
   AdequateLeaderSelectedOwnerPhysicalCorridorTerminalIsExactHandoff,
   PTL, IsaT(600)
   DEF AdequateLeaderTargetSelectedOwnerSemanticHandoffProperty,
       AdequateLeaderTargetSelectedOwnerSemanticHandoffDebt,
       AdequateLeaderTargetSelectedOwnerPhysicalOutcomeProperty,
       AdequateLeaderTargetSelectedOwnerPhysicalEpisodeTerminal,
       AdequateLeaderTargetNonDescentEpisodeAtBudget,
       AdequateLeaderTargetNonDescentEpisodeFrontier,
       AdequateLeaderTargetEpisodeKnownOwnerSet,
       AdequateLeaderTargetUniversalOccurrenceServiceGoal,
       AdequateLeaderTargetOccurrenceRankServiceProperty

AdequateLeaderTargetRetiredOwnerProductiveReentryProperty(
    specification) ==
  specification
    => \A target \in ValidatorIds,
          leaderContext \in ContextRecords,
          leader \in ValidatorIds,
          leaderView \in Views,
          sourceSubject \in Subjects,
          sourceOccurrenceRank \in
            AdequateLeaderTargetOccurrenceRankCarrier,
          retired \in
            SUBSET AdequateLeaderFrozenSubjectSwitchOwnerUniverse(
              target, leaderContext, leader, leaderView),
          owner \in
            AdequateLeaderFrozenCandidateOwnerUniverse(
              target, leaderContext, leader, leaderView, sourceSubject),
          budget \in Nat:
         /\ AdequateLeaderFrozenTargetCorridor(
              target, leaderContext, leader, leaderView)
         /\ owner \notin retired
         /\ retired \cup {owner} \subseteq
              AdequateLeaderTargetDurablyRetiredOwnerIdentitySet(
                target, leaderContext, leader, leaderView)
         /\ budget =
              AdequateLeaderTargetSubjectSwitchRemainingBudget(
                target, leaderContext, leader, leaderView, retired)
           ~> AdequateLeaderTargetOffSubjectRetirementAndReentryGoal(
                target, leaderContext, leader, leaderView,
                sourceSubject, sourceOccurrenceRank,
                owner, retired, budget)

\* Owner-indexed occurrence service alone does not prove that the derived
\* durable-retirement projection reaches the next selected productive owner.
\* Keep that exact carry step explicit.  Its `retired` arguments are now fixed
\* by existing Async lifecycle records, so a provider must prove their
\* monotonicity and the producer-to-next-owner bridge; choosing a fresh
\* mathematical set after re-entry is impossible by definition.  Both
\* control-slot and internal BodyAvailable no-candidate-re-entry safety
\* properties are derived above.
AdequateLeaderTargetSubjectSwitchCarryStepProperty(specification) ==
  specification
    => \A target \in ValidatorIds,
          leaderContext \in ContextRecords,
          leader \in ValidatorIds,
          leaderView \in Views,
          subject \in Subjects,
          occurrenceRank \in
            AdequateLeaderTargetOccurrenceRankCarrier,
          retired \in
            SUBSET AdequateLeaderFrozenSubjectSwitchOwnerUniverse(
              target, leaderContext, leader, leaderView),
          owner \in
            AdequateLeaderFrozenCandidateOwnerUniverse(
              target, leaderContext, leader, leaderView, subject),
          budget \in Nat:
         AdequateLeaderTargetSubjectSwitchEpisodeAtBudget(
           target, leaderContext, leader, leaderView,
           subject, occurrenceRank, owner, retired, budget)
           ~> AdequateLeaderTargetSubjectSwitchBudgetDescentGoal(
                target, leaderContext, leader, leaderView,
                subject, occurrenceRank, owner, retired, budget)

AdequateLeaderTargetSubjectSwitchBudgetDescentProperty(specification) ==
  /\ AdequateLeaderTargetOccurrenceRankServiceProperty(specification)
  /\ AdequateLeaderTargetOffSubjectControlNoReentryProperty(specification)
  /\ AdequateLeaderTargetInternalBodyAvailableNoReentryProperty(
       specification)
  /\ AdequateLeaderTargetDurableRetirementCarryProperty(specification)
  /\ AdequateLeaderTargetSubjectSwitchCarryStepProperty(specification)

\* Freeze the original episode as an anchor, then let only the current episode
\* and its natural-number budget vary.  A lower-budget frontier must retain the
\* original owner in the accumulated retired snapshot.  This makes the release
\* goal common to every induction step, which is the premise required by
\* `WellFoundedLeadsTo`; no temporal-history variable is added to Async state.
\* The measure does not rank views and does not use `AsyncMaximumView` or a TLC
\* view bound.  A higher-view replacement exits the frozen corridor to the
\* separate view-reach kernel; it is never counted as an owner-budget descent.
AdequateLeaderTargetAnchoredSubjectSwitchBudgetFrontier(
    target, leaderContext, leader, leaderView,
    anchorSubject, anchorOccurrenceRank, anchorOwner,
    anchorRetired, anchorBudget, currentBudget) ==
  /\ currentBudget \in Nat
  /\ anchorRetired \subseteq
       AdequateLeaderFrozenSubjectSwitchOwnerUniverse(
         target, leaderContext, leader, leaderView)
  /\ anchorOwner \in
       AdequateLeaderFrozenSubjectSwitchOwnerUniverse(
         target, leaderContext, leader, leaderView)
       \ anchorRetired
  /\ anchorBudget =
       AdequateLeaderTargetSubjectSwitchRemainingBudget(
         target, leaderContext, leader, leaderView, anchorRetired)
  /\ \/ /\ currentBudget = anchorBudget
          /\ AdequateLeaderTargetSubjectSwitchEpisodeAtBudget(
               target, leaderContext, leader, leaderView,
               anchorSubject, anchorOccurrenceRank, anchorOwner,
               anchorRetired, anchorBudget)
     \/ /\ currentBudget < anchorBudget
          /\ \E currentSubject \in Subjects,
                currentOccurrenceRank \in
                  AdequateLeaderTargetOccurrenceRankCarrier,
                currentOwner \in
                  AdequateLeaderFrozenCandidateOwnerUniverse(
                    target, leaderContext, leader,
                    leaderView, currentSubject),
                currentRetired \in
                  SUBSET AdequateLeaderFrozenSubjectSwitchOwnerUniverse(
                    target, leaderContext, leader, leaderView):
               /\ anchorRetired \cup {anchorOwner}
                    \subseteq currentRetired
               /\ AdequateLeaderTargetSubjectSwitchEpisodeAtBudget(
                    target, leaderContext, leader, leaderView,
                    currentSubject, currentOccurrenceRank, currentOwner,
                    currentRetired, currentBudget)

AdequateLeaderTargetAnchoredSubjectSwitchBudgetDescentGoal(
    target, leaderContext, leader, leaderView,
    anchorSubject, anchorOccurrenceRank, anchorOwner,
    anchorRetired, anchorBudget, currentBudget) ==
  \/ AdequateLeaderTargetOffSubjectRetirementAndReentryGoal(
       target, leaderContext, leader, leaderView,
       anchorSubject, anchorOccurrenceRank, anchorOwner,
       anchorRetired, anchorBudget)
  \/ \E lowerBudget \in
       SetLessThan(currentBudget, OpToRel(<, Nat), Nat):
       AdequateLeaderTargetAnchoredSubjectSwitchBudgetFrontier(
         target, leaderContext, leader, leaderView,
         anchorSubject, anchorOccurrenceRank, anchorOwner,
         anchorRetired, anchorBudget, lowerBudget)

AdequateLeaderTargetAnchoredSubjectSwitchBudgetDescentProperty(
    specification) ==
  specification
    => \A target \in ValidatorIds,
          leaderContext \in ContextRecords,
          leader \in ValidatorIds,
          leaderView \in Views,
          anchorSubject \in Subjects,
          anchorOccurrenceRank \in
            AdequateLeaderTargetOccurrenceRankCarrier,
          anchorOwner \in
            AdequateLeaderFrozenCandidateOwnerUniverse(
              target, leaderContext, leader, leaderView, anchorSubject),
          anchorRetired \in
            SUBSET AdequateLeaderFrozenSubjectSwitchOwnerUniverse(
              target, leaderContext, leader, leaderView),
          anchorBudget \in Nat,
          currentBudget \in Nat:
         AdequateLeaderTargetAnchoredSubjectSwitchBudgetFrontier(
           target, leaderContext, leader, leaderView,
           anchorSubject, anchorOccurrenceRank, anchorOwner,
           anchorRetired, anchorBudget, currentBudget)
           ~> AdequateLeaderTargetAnchoredSubjectSwitchBudgetDescentGoal(
                target, leaderContext, leader, leaderView,
                anchorSubject, anchorOccurrenceRank, anchorOwner,
                anchorRetired, anchorBudget, currentBudget)

AdequateLeaderTargetSubjectSwitchClosureProperty(specification) ==
  specification
    => \A target \in ValidatorIds,
          leaderContext \in ContextRecords,
          leader \in ValidatorIds,
          leaderView \in Views,
          subject \in Subjects,
          occurrenceRank \in
            AdequateLeaderTargetOccurrenceRankCarrier,
          retired \in
            SUBSET AdequateLeaderFrozenSubjectSwitchOwnerUniverse(
              target, leaderContext, leader, leaderView),
          owner \in
            AdequateLeaderFrozenCandidateOwnerUniverse(
              target, leaderContext, leader, leaderView, subject),
          budget \in Nat:
         AdequateLeaderTargetSubjectSwitchEpisodeAtBudget(
           target, leaderContext, leader, leaderView,
           subject, occurrenceRank, owner, retired, budget)
           ~> AdequateLeaderTargetOffSubjectRetirementAndReentryGoal(
                target, leaderContext, leader, leaderView,
                subject, occurrenceRank, owner, retired, budget)

THEOREM AdequateLeaderOwnerIndexedServiceProvidesSubjectSwitchBudgetDescent ==
  \A initialContext:
    /\ AdequateLeaderTargetOccurrenceRankServiceProperty(
         AsyncLiveSpecAt(initialContext))
    /\ AdequateLeaderTargetSubjectSwitchCarryStepProperty(
         AsyncLiveSpecAt(initialContext))
      => AdequateLeaderTargetSubjectSwitchBudgetDescentProperty(
           AsyncLiveSpecAt(initialContext))
BY AsyncLiveProvidesAdequateLeaderTargetOffSubjectControlNoReentry,
   AsyncLiveProvidesAdequateLeaderTargetInternalBodyAvailableNoReentry,
   AsyncLiveProvidesAdequateLeaderTargetDurableRetirementCarry,
   PTL
   DEF AdequateLeaderTargetOccurrenceRankServiceProperty,
       AdequateLeaderTargetSubjectSwitchCarryStepProperty,
       AdequateLeaderTargetSubjectSwitchBudgetDescentProperty,
       AdequateLeaderTargetOffSubjectControlNoReentryProperty,
       AdequateLeaderTargetInternalBodyAvailableNoReentryProperty,
       AdequateLeaderTargetDurableRetirementCarryProperty

THEOREM AdequateLeaderSubjectSwitchCarryStepProvidesAnchoredBudgetDescent ==
  \A initialContext:
    AdequateLeaderTargetSubjectSwitchCarryStepProperty(
      AsyncLiveSpecAt(initialContext))
      => AdequateLeaderTargetAnchoredSubjectSwitchBudgetDescentProperty(
           AsyncLiveSpecAt(initialContext))
BY PTL, IsaT(900)
   DEF AdequateLeaderTargetSubjectSwitchCarryStepProperty,
       AdequateLeaderTargetAnchoredSubjectSwitchBudgetDescentProperty,
       AdequateLeaderTargetAnchoredSubjectSwitchBudgetFrontier,
       AdequateLeaderTargetAnchoredSubjectSwitchBudgetDescentGoal,
       AdequateLeaderTargetSubjectSwitchBudgetDescentGoal,
       AdequateLeaderTargetSubjectSwitchEpisodeAdvanceGoal,
       AdequateLeaderTargetOffSubjectRetirementAndReentryGoal,
       AdequateLeaderTargetSubjectSwitchEpisodeAtBudget,
       AdequateLeaderTargetCarriedOwnerEpisodeAtBudget,
       AdequateLeaderTargetSubjectSwitchDiscoveredOwnerSet,
       SetLessThan, OpToRel

THEOREM AdequateLeaderSubjectSwitchBudgetDescentClosesNamedOwnerEpisode ==
  \A initialContext:
    AdequateLeaderTargetSubjectSwitchBudgetDescentProperty(
      AsyncLiveSpecAt(initialContext))
      => AdequateLeaderTargetSubjectSwitchClosureProperty(
           AsyncLiveSpecAt(initialContext))
BY AdequateLeaderSubjectSwitchCarryStepProvidesAnchoredBudgetDescent,
   NatLessThanWellFounded, WellFoundedLeadsTo, PTL
   DEF AdequateLeaderTargetSubjectSwitchBudgetDescentProperty,
       AdequateLeaderTargetSubjectSwitchClosureProperty,
       AdequateLeaderTargetAnchoredSubjectSwitchBudgetDescentProperty,
       AdequateLeaderTargetAnchoredSubjectSwitchBudgetFrontier,
       AdequateLeaderTargetAnchoredSubjectSwitchBudgetDescentGoal

(***************************************************************************
Finite non-descent composition.

The indexed bridge below is the temporal conclusion of occurrence service and
the rank-indexed producer corridor.  It does not call replenishment progress.
Starting from one episode frontier at `Cardinality(U \ known)`, service reaches
the strict occurrence goal, the separately handled off-subject drain exit, or
a state which exposes `discovered`; the target explicitly carries
`known2 = known \cup discovered` and a strictly smaller complement budget.
The subsequent projection forgets only the identity set, keeps the smaller
natural-number budget, and applies finite Nat induction.

Target-subject wire identities already have exact consumed-or-strictly-
advanced retirement.  Candidate identities now have bounded transient and
terminal memory plus the exact internal BodyAvailable preflight bridge needed
to exclude the observed A -> B -> A resurrection.  The indexed composition
below combines those safety facts with the concrete occurrence-service/
producer corridor: a discovered identity advances only the frozen known set
and its finite complement budget; replenishment itself is never a progress
goal.
***************************************************************************)
AdequateLeaderTargetNonDescentKnownAdvanceProperty(specification) ==
  specification
    => \A target \in ValidatorIds,
          leaderContext \in ContextRecords,
          leader \in ValidatorIds,
          leaderView \in Views,
          subject \in Subjects,
          sourceOccurrenceRank \in
            AdequateLeaderTargetOccurrenceRankCarrier,
          known \in
            SUBSET AdequateLeaderFrozenOwnerUniverse(
              target, leaderContext, leader, leaderView, subject),
          budget \in Nat,
          owner \in
            AdequateLeaderFrozenCandidateOwnerUniverse(
              target, leaderContext, leader, leaderView, subject):
         /\ AdequateLeaderTargetProtocolSubjectSource(
              target, leaderContext, leader, leaderView, subject)
         /\ AdequateLeaderTargetNonDescentEpisodeAtBudget(
              target, leaderContext, leader, leaderView,
              subject, sourceOccurrenceRank, known, budget)
         /\ AdequateLeaderTargetOccurrenceOwnerCarried(
              target, leaderContext, leader, leaderView,
              subject, sourceOccurrenceRank, known, owner)
           ~> (AdequateLeaderTargetOccurrenceRankServiceExitGoal(
                 target, leaderContext, leader, leaderView,
                 subject, sourceOccurrenceRank, owner)
                \/ AdequateLeaderTargetCarriedNonDescentKnownAdvanceGoal(
                     target, leaderContext, leader, leaderView,
                     subject, sourceOccurrenceRank,
                     known, budget, owner))

\* The producer endpoint returned by occurrence service retains the episode
\* coordinates.  `known` and its complement budget range over the immutable
\* frozen universe, so the only state-dependent clause to recover is the live
\* owner subset supplied explicitly by the service property.
THEOREM AdequateLeaderTargetKnownProducerRetainsEpisodeBudget ==
  \A target, leaderContext, leader, leaderView,
     subject, sourceOccurrenceRank, known, budget:
    /\ AdequateLeaderTargetEpisodeKnownOwnerSet(
         target, leaderContext, leader, leaderView, subject, known)
    /\ budget =
         AdequateLeaderTargetNonDescentEpisodeBudget(
           target, leaderContext, leader, leaderView, subject, known)
    /\ AdequateLeaderTargetProducerTransportResidualAtOccurrence(
         target, leaderContext, leader, leaderView,
         subject, sourceOccurrenceRank)
    /\ AdequateLeaderTargetLiveOwnerIdentitySet(
         target, leaderContext, leader, leaderView, subject)
         \subseteq known
    => AdequateLeaderTargetNonDescentEpisodeAtBudget(
         target, leaderContext, leader, leaderView,
         subject, sourceOccurrenceRank, known, budget)
BY Isa
   DEF AdequateLeaderTargetNonDescentEpisodeAtBudget,
       AdequateLeaderTargetNonDescentEpisodeFrontier,
       AdequateLeaderTargetOccurrenceEpisodeActive

\* One episode starts either at a same/higher ranked owner or inside the
\* producer corridor.  Occurrence service and producer closure therefore
\* compose to strict descent or a genuine finite-universe discovery.  The
\* latter is converted immediately to `known U discovered` with a strictly
\* smaller complement budget; no replenishment action is used as progress.
THEOREM AdequateLeaderOccurrenceAndProducerClosureAdvanceKnownBudget ==
  \A initialContext:
    /\ AdequateLeaderTargetOccurrenceRankServiceProperty(
         AsyncLiveSpecAt(initialContext))
    /\ AdequateLeaderTargetProducerTransportOccurrenceClosureProperty(
         AsyncLiveSpecAt(initialContext))
    => AdequateLeaderTargetNonDescentKnownAdvanceProperty(
         AsyncLiveSpecAt(initialContext))
BY AsyncSpecAlwaysStrongTypeInvariant,
   AsyncLiveSpecProjectsAsyncSpec,
   AdequateLeaderFrozenOwnerUniverseIsPrimeInvariant,
   AdequateLeaderTargetKnownProducerRetainsEpisodeBudget,
   AdequateLeaderCarriedNonDescentResidualAdvancesKnownBudget,
   PTL
   DEF AdequateLeaderTargetOccurrenceRankServiceProperty,
       AdequateLeaderTargetProducerTransportOccurrenceClosureProperty,
       AdequateLeaderTargetUniversalOccurrenceServiceGoal,
       AdequateLeaderTargetProductiveOccurrenceServiceGoal,
       AdequateLeaderTargetOffSubjectOccurrenceDrainGoal,
       AdequateLeaderTargetOccurrenceRankServiceExitGoal,
       AdequateLeaderTargetOccurrenceOwnerCarried,
       AdequateLeaderTargetCarriedNonDescentEpisodeResidual,
       AdequateLeaderTargetCarriedNonDescentKnownAdvanceGoal,
       AdequateLeaderTargetNonDescentKnownAdvanceProperty,
       AdequateLeaderTargetNonDescentEpisodeAtBudget,
       AdequateLeaderTargetNonDescentEpisodeFrontier,
       AdequateLeaderTargetEpisodeKnownOwnerSet,
       AdequateLeaderTargetNonDescentEpisodeBudget,
       AdequateLeaderTargetOccurrenceEpisodeActive

\* Compatibility name retained for downstream proof imports, but `known` is
\* now an immutable episode parameter.  A caller cannot lower the complement
\* budget in the same state by existentially choosing a larger known set.
AdequateLeaderTargetNonDescentEpisodeBudgetFrontier(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank, owner, known, budget) ==
  /\ known
       \in SUBSET AdequateLeaderFrozenOwnerUniverse(
            target, leaderContext, leader, leaderView, subject)
  /\ AdequateLeaderTargetNonDescentEpisodeAtBudget(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank, known, budget)
  /\ AdequateLeaderTargetOccurrenceOwnerCarried(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank, known, owner)

AdequateLeaderTargetServiceExitOrBudgetDescentGoal(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank, owner, known, budget) ==
  \/ AdequateLeaderTargetOccurrenceRankServiceExitGoal(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank, owner)
  \/ \E discovered,
       known2
         \in SUBSET AdequateLeaderFrozenOwnerUniverse(
              target, leaderContext, leader, leaderView, subject),
       lowerBudget \in
         SetLessThan(budget, OpToRel(<, Nat), Nat):
       /\ discovered =
            AdequateLeaderTargetNonDescentDiscoveredOwnerIdentitySet(
              target, leaderContext, leader, leaderView, subject, known)
       /\ discovered # {}
       /\ known2 = known \cup discovered
       /\ AdequateLeaderTargetNonDescentEpisodeBudgetFrontier(
            target, leaderContext, leader, leaderView,
            subject, sourceOccurrenceRank, owner, known2, lowerBudget)

AdequateLeaderTargetNonDescentEpisodeBudgetDescentProperty(
    specification) ==
  specification
    => \A target \in ValidatorIds,
          leaderContext \in ContextRecords,
          leader \in ValidatorIds,
          leaderView \in Views,
          subject \in Subjects,
          sourceOccurrenceRank \in
            AdequateLeaderTargetOccurrenceRankCarrier,
          owner \in
            AdequateLeaderFrozenCandidateOwnerUniverse(
              target, leaderContext, leader, leaderView, subject),
          known
            \in SUBSET AdequateLeaderFrozenOwnerUniverse(
                 target, leaderContext, leader, leaderView, subject),
          budget \in Nat:
         /\ AdequateLeaderTargetProtocolSubjectSource(
              target, leaderContext, leader, leaderView, subject)
         /\ AdequateLeaderTargetNonDescentEpisodeBudgetFrontier(
              target, leaderContext, leader, leaderView,
              subject, sourceOccurrenceRank, owner, known, budget)
           ~> AdequateLeaderTargetServiceExitOrBudgetDescentGoal(
                target, leaderContext, leader, leaderView,
                subject, sourceOccurrenceRank, owner, known, budget)

AdequateLeaderTargetNonDescentEpisodeClosureProperty(specification) ==
  specification
    => \A target \in ValidatorIds,
          leaderContext \in ContextRecords,
          leader \in ValidatorIds,
          leaderView \in Views,
          subject \in Subjects,
          sourceOccurrenceRank \in
            AdequateLeaderTargetOccurrenceRankCarrier,
          owner \in
            AdequateLeaderFrozenCandidateOwnerUniverse(
              target, leaderContext, leader, leaderView, subject),
          known
            \in SUBSET AdequateLeaderFrozenOwnerUniverse(
                 target, leaderContext, leader, leaderView, subject),
          budget \in Nat:
         /\ AdequateLeaderTargetProtocolSubjectSource(
              target, leaderContext, leader, leaderView, subject)
         /\ AdequateLeaderTargetNonDescentEpisodeBudgetFrontier(
              target, leaderContext, leader, leaderView,
              subject, sourceOccurrenceRank, owner, known, budget)
           ~> AdequateLeaderTargetOccurrenceRankServiceExitGoal(
                target, leaderContext, leader, leaderView,
                subject, sourceOccurrenceRank, owner)

AdequateLeaderTargetComposedRankDescentProperty(specification) ==
  /\ AdequateLeaderTargetOccurrenceRankServiceProperty(specification)
  /\ AdequateLeaderTargetProducerTransportOccurrenceClosureProperty(
       specification)
  /\ AdequateLeaderTargetNonDescentKnownAdvanceProperty(specification)

THEOREM AdequateLeaderOccurrenceAndProducerClosureProvideComposedRankDescent ==
  \A initialContext:
    /\ AdequateLeaderTargetOccurrenceRankServiceProperty(
         AsyncLiveSpecAt(initialContext))
    /\ AdequateLeaderTargetProducerTransportOccurrenceClosureProperty(
         AsyncLiveSpecAt(initialContext))
    => AdequateLeaderTargetComposedRankDescentProperty(
         AsyncLiveSpecAt(initialContext))
BY AdequateLeaderOccurrenceAndProducerClosureAdvanceKnownBudget
   DEF AdequateLeaderTargetComposedRankDescentProperty

THEOREM AdequateLeaderTargetOccurrenceFrontierStartsFiniteEpisode ==
  \A target, leaderContext, leader, leaderView,
     subject, sourceOccurrenceRank, owner:
    /\ AsyncStrongTypeInvariant
    /\ AdequateLeaderFrozenTargetCorridor(
         target, leaderContext, leader, leaderView)
    /\ AdequateLeaderTargetOccurrenceRankFrontier(
         target, leaderContext, leader, leaderView,
         subject, sourceOccurrenceRank)
    /\ AdequateLeaderTargetOccurrenceOwnerSelected(
         target, leaderContext, leader, leaderView,
         subject, sourceOccurrenceRank, owner)
    => \/ AdequateLeaderTargetStrictOccurrenceDescentGoal(
            target, leaderContext, leader, leaderView,
            subject, sourceOccurrenceRank)
       \/ \E known
              \in SUBSET AdequateLeaderFrozenOwnerUniverse(
                   target, leaderContext, leader, leaderView, subject),
            budget \in Nat:
            /\ known =
                 AdequateLeaderTargetLiveOwnerIdentitySet(
                   target, leaderContext, leader, leaderView, subject)
            /\ AdequateLeaderTargetNonDescentEpisodeBudgetFrontier(
                 target, leaderContext, leader, leaderView,
                 subject, sourceOccurrenceRank, owner, known, budget)
BY AdequateLeaderTargetCurrentOwnersInitializeKnownEpisode,
   AdequateLeaderTargetNonDescentEpisodeBudgetIsFiniteAndCoalesced,
   IsaT(240)
   DEF AdequateLeaderTargetNonDescentEpisodeBudgetFrontier,
       AdequateLeaderTargetNonDescentEpisodeAtBudget,
       AdequateLeaderTargetNonDescentEpisodeFrontier,
       AdequateLeaderTargetOccurrenceEpisodeActive,
       AdequateLeaderTargetSameOrHigherOccurrenceFrontier,
       AdequateLeaderTargetEpisodeStartsWithCurrentOwners,
       AdequateLeaderTargetOccurrenceOwnerCarried,
       AdequateLeaderTargetOccurrenceOwnerSelected,
       AdequateLeaderTargetOccurrenceOwnerIdentitySet,
       AdequateLeaderTargetLiveOwnerIdentitySet

THEOREM AdequateLeaderKnownAdvanceProjectsToServiceExitBudgetDescent ==
  \A initialContext:
    AdequateLeaderTargetNonDescentKnownAdvanceProperty(
      AsyncLiveSpecAt(initialContext))
      => AdequateLeaderTargetNonDescentEpisodeBudgetDescentProperty(
           AsyncLiveSpecAt(initialContext))
BY PTL
   DEF AdequateLeaderTargetNonDescentKnownAdvanceProperty,
       AdequateLeaderTargetCarriedNonDescentKnownAdvanceGoal,
       AdequateLeaderTargetOccurrenceOwnerCarried,
       AdequateLeaderTargetNonDescentEpisodeBudgetDescentProperty,
       AdequateLeaderTargetNonDescentEpisodeBudgetFrontier,
       AdequateLeaderTargetServiceExitOrBudgetDescentGoal,
       SetLessThan, OpToRel

THEOREM AdequateLeaderFiniteBudgetDescentClosesNonDescentEpisode ==
  \A initialContext:
    AdequateLeaderTargetNonDescentEpisodeBudgetDescentProperty(
      AsyncLiveSpecAt(initialContext))
      => AdequateLeaderTargetNonDescentEpisodeClosureProperty(
           AsyncLiveSpecAt(initialContext))
BY NatLessThanWellFounded, WellFoundedLeadsTo
   DEF AdequateLeaderTargetNonDescentEpisodeBudgetDescentProperty,
       AdequateLeaderTargetNonDescentEpisodeClosureProperty,
       AdequateLeaderTargetServiceExitOrBudgetDescentGoal

AdequateLeaderTargetRankServiceExitProperty(specification) ==
  specification
    => \A target \in ValidatorIds,
          leaderContext \in ContextRecords,
          leader \in ValidatorIds,
          leaderView \in Views,
          subject \in Subjects,
          occurrenceRank \in AdequateLeaderTargetOccurrenceRankCarrier,
          owner \in
            AdequateLeaderFrozenCandidateOwnerUniverse(
              target, leaderContext, leader, leaderView, subject):
         /\ AdequateLeaderTargetProtocolSubjectSource(
              target, leaderContext, leader, leaderView, subject)
         /\ AdequateLeaderTargetOccurrenceRankFrontier(
              target, leaderContext, leader,
              leaderView, subject, occurrenceRank)
         /\ AdequateLeaderTargetOccurrenceOwnerSelected(
              target, leaderContext, leader, leaderView,
              subject, occurrenceRank, owner)
           ~> AdequateLeaderTargetOccurrenceRankServiceExitGoal(
                target, leaderContext, leader,
                leaderView, subject, occurrenceRank, owner)

(***************************************************************************
Release-boundary provider inventory.

The finite owner and occurrence inductions below are proved compositions; they
must not be mistaken for providers of their temporal premises.  The generic
retained packet, ordinary ingress/runner, and timeout-quorum handoff is now
provided by
`AsyncLiveProvidesAdequateLeaderOpenPhysicalResidualConvergence`.  On the
current transition relation the remaining adequate-leader providers are
exactly:

  * `AdequateLeaderViewReachCompositionProperty` for unbounded-Nat rotating
    view reach (with no `AsyncMaximumView` rank);
  * `AdequateLeaderTargetCorridorEntryProperty` for simultaneous fresh
    target/leader ownership at one adequate view;
  * `AdequateLeaderTargetProducerTransportClosureProperty` and its
    occurrence-indexed counterpart for missing/not-yet-due producer debt;
  * `AdequateLeaderTargetSelectedOwnerSemanticHandoffProperty` for the exact
    post-drain producer/retirement handoff.  The finite physical identity
    drain is concrete above and this narrower seam conditionally supplies
    `AdequateLeaderTargetOccurrenceRankServiceProperty`.

The durable retired-set carry step is derived from named occurrence service
plus the unindexed producer closure.  The final induction uses the
lexicographic pair of the global frozen-owner budget and occurrence rank: a
subject switch may lower only the first component, while same-subject service
may lower only the second.

Corridor exit is merely a handoff to the second item.  It is neither Decision
nor a lower occurrence/owner rank.  No aggregate leaf is promoted here.
***************************************************************************)

THEOREM AdequateLeaderComposedRankDescentClosesOccurrenceService ==
  \A initialContext:
    AdequateLeaderTargetComposedRankDescentProperty(
      AsyncLiveSpecAt(initialContext))
      => AdequateLeaderTargetRankServiceExitProperty(
           AsyncLiveSpecAt(initialContext))
BY AsyncSpecAlwaysStrongTypeInvariant,
   AsyncLiveSpecProjectsAsyncSpec,
   AdequateLeaderTargetOccurrenceFrontierStartsFiniteEpisode,
   AdequateLeaderKnownAdvanceProjectsToServiceExitBudgetDescent,
   AdequateLeaderFiniteBudgetDescentClosesNonDescentEpisode,
   PTL
   DEF AdequateLeaderTargetComposedRankDescentProperty,
       AdequateLeaderTargetOccurrenceRankServiceProperty,
       AdequateLeaderTargetProducerTransportOccurrenceClosureProperty,
       AdequateLeaderTargetUniversalOccurrenceServiceGoal,
       AdequateLeaderTargetProductiveOccurrenceServiceGoal,
       AdequateLeaderTargetOffSubjectOccurrenceDrainGoal,
       AdequateLeaderTargetNonDescentKnownAdvanceProperty,
       AdequateLeaderTargetNonDescentEpisodeClosureProperty,
       AdequateLeaderTargetNonDescentEpisodeBudgetFrontier,
       AdequateLeaderTargetNonDescentEpisodeAtBudget,
       AdequateLeaderTargetRankServiceExitProperty

AdequateLeaderTargetSemanticCompositionProperty(specification) ==
  /\ AdequateLeaderTargetCorridorEntryProperty(specification)
  /\ AdequateLeaderTargetProducerTransportClosureProperty(specification)
  /\ AdequateLeaderTargetOccurrenceRankServiceProperty(specification)
  /\ AdequateLeaderTargetProducerTransportOccurrenceClosureProperty(
       specification)

AdequateLeaderSemanticCompositionProperty(specification) ==
  /\ AdequateLeaderViewReachCompositionProperty(specification)
  /\ AdequateLeaderTargetSemanticCompositionProperty(specification)

\* The bounded candidate marker, same-height preservation, strict-advance
\* reclamation, and finite carrier bridge are discharged above.  They support
\* the owner-by-occurrence episode construction below.  This module does not
\* turn a corridor-exit handoff into Decision; the continuation layer must
\* supply that exact-target bridge.

THEOREM AdequateLeaderTargetSemanticRankOrderingWellFounded ==
  IsWellFoundedOn(
    AdequateLeaderTargetSemanticRankOrdering,
    AdequateLeaderTargetSemanticRankCarrier)
BY NatLessThanWellFounded, IsWellFoundedOnSubset,
   WFLexPairOrdering, SMT
   DEF AdequateLeaderTargetSemanticRankOrdering,
       AdequateLeaderTargetSemanticRankCarrier

THEOREM AdequateLeaderTargetOccurrenceRankOrderingWellFounded ==
  IsWellFoundedOn(
    AdequateLeaderTargetOccurrenceRankOrdering,
    AdequateLeaderTargetOccurrenceRankCarrier)
BY AdequateLeaderTargetSemanticRankOrderingWellFounded,
   NatLessThanWellFounded, WFLexPairOrdering
   DEF AdequateLeaderTargetOccurrenceRankOrdering,
       AdequateLeaderTargetOccurrenceRankCarrier

\* Subject replacement is ordered outside occurrence service.  The first
\* component is the complement of the exact durable retired-owner projection;
\* the second is the Decision-pipeline occurrence rank.  A subject switch may
\* choose any occurrence rank only after strictly consuming the first
\* component.  At equal owner budget, only a strict occurrence decrease is a
\* descent.  Corridor exit is absent from this ordering.
AdequateLeaderTargetProductiveEpisodeRankCarrier ==
  Nat \X AdequateLeaderTargetOccurrenceRankCarrier

AdequateLeaderTargetProductiveEpisodeRankOrdering ==
  LexPairOrdering(
    OpToRel(<, Nat),
    AdequateLeaderTargetOccurrenceRankOrdering,
    Nat,
    AdequateLeaderTargetOccurrenceRankCarrier)

THEOREM AdequateLeaderTargetProductiveEpisodeRankOrderingWellFounded ==
  IsWellFoundedOn(
    AdequateLeaderTargetProductiveEpisodeRankOrdering,
    AdequateLeaderTargetProductiveEpisodeRankCarrier)
BY NatLessThanWellFounded,
   AdequateLeaderTargetOccurrenceRankOrderingWellFounded,
   WFLexPairOrdering
   DEF AdequateLeaderTargetProductiveEpisodeRankOrdering,
       AdequateLeaderTargetProductiveEpisodeRankCarrier

AdequateLeaderTargetProductiveEpisodeRankFrontier(
    target, leaderContext, leader, leaderView, episodeRank) ==
  /\ episodeRank \in
       AdequateLeaderTargetProductiveEpisodeRankCarrier
  /\ \E subject \in Subjects,
        owner \in
          AdequateLeaderFrozenCandidateOwnerUniverse(
            target, leaderContext, leader, leaderView, subject),
        retired \in
          SUBSET AdequateLeaderFrozenSubjectSwitchOwnerUniverse(
            target, leaderContext, leader, leaderView):
       AdequateLeaderTargetProductiveOwnerEpisodeAtBudget(
         target, leaderContext, leader, leaderView,
         subject, episodeRank[2], owner, retired, episodeRank[1])

AdequateLeaderTargetProductiveEpisodeStrictDescentGoal(
    target, leaderContext, leader, leaderView, sourceEpisodeRank) ==
  \/ AdequateLeaderTargetFrozenCorridorTerminalGoal(
       target, leaderContext, leader, leaderView)
  \/ \E lowerEpisodeRank \in
       SetLessThan(
         sourceEpisodeRank,
         AdequateLeaderTargetProductiveEpisodeRankOrdering,
         AdequateLeaderTargetProductiveEpisodeRankCarrier):
       AdequateLeaderTargetProductiveEpisodeRankFrontier(
         target, leaderContext, leader, leaderView, lowerEpisodeRank)

AdequateLeaderTargetProductiveEpisodeRankStepProperty(specification) ==
  specification
    => \A target \in ValidatorIds,
          leaderContext \in ContextRecords,
          leader \in ValidatorIds,
          leaderView \in Views,
          episodeRank \in
            AdequateLeaderTargetProductiveEpisodeRankCarrier:
         AdequateLeaderTargetProductiveEpisodeRankFrontier(
           target, leaderContext, leader, leaderView, episodeRank)
           ~> AdequateLeaderTargetProductiveEpisodeStrictDescentGoal(
                target, leaderContext, leader, leaderView, episodeRank)

AdequateLeaderTargetProductiveEpisodeClosureProperty(specification) ==
  specification
    => \A target \in ValidatorIds,
          leaderContext \in ContextRecords,
          leader \in ValidatorIds,
          leaderView \in Views,
          episodeRank \in
            AdequateLeaderTargetProductiveEpisodeRankCarrier:
         AdequateLeaderTargetProductiveEpisodeRankFrontier(
           target, leaderContext, leader, leaderView, episodeRank)
           ~> AdequateLeaderTargetFrozenCorridorTerminalGoal(
                target, leaderContext, leader, leaderView)

THEOREM AdequateLeaderProductiveEpisodeRankStepClosesFrozenCorridor ==
  \A specification:
    AdequateLeaderTargetProductiveEpisodeRankStepProperty(specification)
      => AdequateLeaderTargetProductiveEpisodeClosureProperty(specification)
BY AdequateLeaderTargetProductiveEpisodeRankOrderingWellFounded,
   WellFoundedLeadsTo
   DEF AdequateLeaderTargetProductiveEpisodeRankStepProperty,
       AdequateLeaderTargetProductiveEpisodeClosureProperty,
       AdequateLeaderTargetProductiveEpisodeStrictDescentGoal

THEOREM AdequateLeaderProtocolOccurrenceFrontierHasMinimalOwnerEpisode ==
  \A target, leaderContext, leader, leaderView, subject:
    /\ AsyncStrongTypeInvariant
    /\ AsyncCandidateServiceTombstoneLifecycleInvariant
    /\ AdequateLeaderFrozenTargetCorridor(
         target, leaderContext, leader, leaderView)
    /\ AdequateLeaderTargetProtocolSubjectSource(
         target, leaderContext, leader, leaderView, subject)
    /\ AdequateLeaderTargetOccurrenceFrontierRankSet(
         target, leaderContext, leader, leaderView, subject) # {}
    => \E occurrenceRank \in
             AdequateLeaderTargetOccurrenceRankCarrier,
           owner \in
             AdequateLeaderFrozenCandidateOwnerUniverse(
               target, leaderContext, leader, leaderView, subject),
           retired \in
             SUBSET AdequateLeaderFrozenSubjectSwitchOwnerUniverse(
               target, leaderContext, leader, leaderView),
           budget \in Nat:
         /\ retired =
              AdequateLeaderTargetDurablyRetiredOwnerIdentitySet(
                target, leaderContext, leader, leaderView)
         /\ budget =
              AdequateLeaderTargetSubjectSwitchRemainingBudget(
                target, leaderContext, leader, leaderView, retired)
         /\ AdequateLeaderTargetProductiveOwnerEpisodeAtBudget(
              target, leaderContext, leader, leaderView,
              subject, occurrenceRank, owner, retired, budget)
PROOF
  <1>1. ASSUME NEW target, NEW leaderContext, NEW leader,
                NEW leaderView, NEW subject,
                /\ AsyncStrongTypeInvariant
                /\ AsyncCandidateServiceTombstoneLifecycleInvariant
                /\ AdequateLeaderFrozenTargetCorridor(
                     target, leaderContext, leader, leaderView)
                /\ AdequateLeaderTargetProtocolSubjectSource(
                     target, leaderContext, leader, leaderView, subject)
                /\ AdequateLeaderTargetOccurrenceFrontierRankSet(
                     target, leaderContext, leader,
                     leaderView, subject) # {}
         PROVE \E occurrenceRank \in
                       AdequateLeaderTargetOccurrenceRankCarrier,
                     owner \in
                       AdequateLeaderFrozenCandidateOwnerUniverse(
                         target, leaderContext, leader,
                         leaderView, subject),
                     retired \in
                       SUBSET AdequateLeaderFrozenSubjectSwitchOwnerUniverse(
                         target, leaderContext, leader, leaderView),
                     budget \in Nat:
                   /\ retired =
                        AdequateLeaderTargetDurablyRetiredOwnerIdentitySet(
                          target, leaderContext, leader, leaderView)
                   /\ budget =
                        AdequateLeaderTargetSubjectSwitchRemainingBudget(
                          target, leaderContext, leader,
                          leaderView, retired)
                   /\ AdequateLeaderTargetProductiveOwnerEpisodeAtBudget(
                        target, leaderContext, leader, leaderView,
                        subject, occurrenceRank, owner, retired, budget)
    <2>1. AdequateLeaderTargetOccurrenceFrontierRankSet(
             target, leaderContext, leader, leaderView, subject)
             \subseteq AdequateLeaderTargetOccurrenceRankCarrier
      BY DEF AdequateLeaderTargetOccurrenceFrontierRankSet
    <2>2. \E occurrenceRank \in
                 AdequateLeaderTargetOccurrenceFrontierRankSet(
                   target, leaderContext, leader, leaderView, subject):
               \A lowerOccurrenceRank \in
                    AdequateLeaderTargetOccurrenceFrontierRankSet(
                      target, leaderContext, leader, leaderView, subject):
                 <<lowerOccurrenceRank, occurrenceRank>>
                   \notin AdequateLeaderTargetOccurrenceRankOrdering
      BY AdequateLeaderTargetOccurrenceRankOrderingWellFounded,
         WFMin, <1>1, <2>1
    <2> QED BY <1>1, <2>2,
         AdequateLeaderOccurrenceFrontierHasSelectedFrozenOwner,
         AdequateLeaderDurablyRetiredOwnersAreNotLiveInFrozenCorridor,
         AdequateLeaderFrozenSubjectSwitchOwnerUniverseIsFinite,
         AdequateLeaderTargetSubjectSwitchBudgetIsFinite,
         FS_Subset, IsaT(1200)
         DEF AdequateLeaderTargetOccurrenceFrontierRankSet,
             AdequateLeaderTargetStrictOccurrenceDescentGoal,
             AdequateLeaderTargetProductiveOwnerEpisodeAtBudget,
             AdequateLeaderTargetCarriedOwnerEpisodeAtBudget,
             AdequateLeaderTargetSubjectSwitchRetiredOwnerSet,
             AdequateLeaderTargetDurablyRetiredOwnerIdentitySet,
             AdequateLeaderTargetSubjectSwitchRemainingBudget,
             AdequateLeaderTargetProtocolSubjectSource,
             SetLessThan
  <1> QED BY <1>1

THEOREM AdequateLeaderRetiredOwnerAndProtocolFrontierEstablishReentry ==
  \A target, leaderContext, leader, leaderView,
     sourceSubject, sourceOccurrenceRank, retired, budget, owner,
     nextSubject:
    /\ AsyncStrongTypeInvariant
    /\ AsyncCandidateServiceTombstoneLifecycleInvariant
    /\ AdequateLeaderFrozenTargetCorridor(
         target, leaderContext, leader, leaderView)
    /\ sourceSubject \in Subjects
    /\ sourceOccurrenceRank \in
         AdequateLeaderTargetOccurrenceRankCarrier
    /\ owner \in
         AdequateLeaderFrozenCandidateOwnerUniverse(
           target, leaderContext, leader, leaderView, sourceSubject)
         \ retired
    /\ retired \subseteq
         AdequateLeaderFrozenSubjectSwitchOwnerUniverse(
           target, leaderContext, leader, leaderView)
    /\ retired \cup {owner} \subseteq
         AdequateLeaderTargetDurablyRetiredOwnerIdentitySet(
           target, leaderContext, leader, leaderView)
    /\ budget =
         AdequateLeaderTargetSubjectSwitchRemainingBudget(
           target, leaderContext, leader, leaderView, retired)
    /\ AdequateLeaderTargetProtocolSubjectSource(
         target, leaderContext, leader, leaderView, nextSubject)
    /\ AdequateLeaderTargetOccurrenceFrontierRankSet(
         target, leaderContext, leader, leaderView, nextSubject) # {}
    => AdequateLeaderTargetOffSubjectRetirementAndReentryGoal(
         target, leaderContext, leader, leaderView,
         sourceSubject, sourceOccurrenceRank, owner, retired, budget)
BY AdequateLeaderProtocolOccurrenceFrontierHasMinimalOwnerEpisode,
   AdequateLeaderSubjectSwitchNamedOwnerStrictlyConsumesBudget,
   IsaT(900)
   DEF AdequateLeaderTargetOffSubjectRetirementAndReentryGoal,
       AdequateLeaderTargetSubjectSwitchRetiredOwnerSet,
       AdequateLeaderTargetDurablyRetiredOwnerIdentitySet,
       AdequateLeaderTargetSubjectSwitchRemainingBudget,
       SetLessThan, OpToRel

THEOREM AdequateLeaderProducerClosureProvidesRetiredOwnerReentry ==
  \A initialContext:
    AdequateLeaderTargetProducerTransportClosureProperty(
      AsyncLiveSpecAt(initialContext))
      => AdequateLeaderTargetRetiredOwnerProductiveReentryProperty(
           AsyncLiveSpecAt(initialContext))
BY AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysCandidateServiceTombstoneLifecycle,
   AsyncLiveSpecProjectsAsyncSpec,
   AsyncLiveProvidesAdequateLeaderTargetDurableRetirementCarry,
   AdequateLeaderDurableRetirementCarryLiftsToFrozenSnapshots,
   AdequateLeaderFrozenCorridorHasProductiveSubjectReentry,
   AdequateLeaderRetiredOwnerAndProtocolFrontierEstablishReentry,
   PTL, IsaT(1200)
   DEF AdequateLeaderTargetProducerTransportClosureProperty,
       AdequateLeaderTargetRetiredOwnerProductiveReentryProperty,
       AdequateLeaderTargetProductiveSubjectReentryGoal,
       AdequateLeaderTargetProductiveSubjectOpenFrontier,
       AdequateLeaderTargetOpenFrontier,
       AdequateLeaderTargetOccurrenceFrontierRankSet,
       AdequateLeaderTargetDurableRetirementSnapshotCarryProperty,
       AdequateLeaderTargetOffSubjectRetirementAndReentryGoal

THEOREM AdequateLeaderOccurrenceServiceAndReentryProvideSubjectSwitchCarry ==
  \A initialContext:
    /\ AdequateLeaderTargetOccurrenceRankServiceProperty(
         AsyncLiveSpecAt(initialContext))
    /\ AdequateLeaderTargetRetiredOwnerProductiveReentryProperty(
         AsyncLiveSpecAt(initialContext))
      => AdequateLeaderTargetSubjectSwitchCarryStepProperty(
           AsyncLiveSpecAt(initialContext))
BY AsyncSpecAlwaysStrongTypeInvariant,
   AsyncLiveSpecProjectsAsyncSpec,
   AsyncLiveProvidesAdequateLeaderTargetDurableRetirementCarry,
   AdequateLeaderDurableRetirementCarryLiftsToFrozenSnapshots,
   AdequateLeaderSubjectSwitchEpisodeStartsNamedOwnerService,
   AdequateLeaderClosedOccurrenceOwnerIsDurablyRetired,
   PTL, IsaT(1200)
   DEF AdequateLeaderTargetOccurrenceRankServiceProperty,
       AdequateLeaderTargetUniversalOccurrenceServiceGoal,
       AdequateLeaderTargetProductiveOccurrenceServiceGoal,
       AdequateLeaderTargetOffSubjectOccurrenceDrainGoal,
       AdequateLeaderTargetRetiredOwnerProductiveReentryProperty,
       AdequateLeaderTargetSubjectSwitchCarryStepProperty,
       AdequateLeaderTargetSubjectSwitchBudgetDescentGoal,
       AdequateLeaderTargetSubjectSwitchEpisodeAtBudget,
       AdequateLeaderTargetCarriedOwnerEpisodeAtBudget,
       AdequateLeaderTargetSubjectSwitchRetiredOwnerSet,
       AdequateLeaderTargetDurableRetirementSnapshotCarryProperty,
       AdequateLeaderTargetProtocolSubjectSource

THEOREM AdequateLeaderOccurrenceAndProducerClosureProvideSubjectSwitchCarry ==
  \A initialContext:
    /\ AdequateLeaderTargetOccurrenceRankServiceProperty(
         AsyncLiveSpecAt(initialContext))
    /\ AdequateLeaderTargetProducerTransportClosureProperty(
         AsyncLiveSpecAt(initialContext))
      => AdequateLeaderTargetSubjectSwitchCarryStepProperty(
           AsyncLiveSpecAt(initialContext))
BY AdequateLeaderProducerClosureProvidesRetiredOwnerReentry,
   AdequateLeaderOccurrenceServiceAndReentryProvideSubjectSwitchCarry

\* These are exactly the state invariants used by the local rank projection.
\* Keeping them as a property parameter makes the final theorem reusable by
\* an indexed Async instance without importing one-height/application or
\* all-joined assumptions.
AdequateLeaderTargetProofInvariantsProperty(specification) ==
  specification
    => [](/\ AsyncStrongTypeInvariant
          /\ AsyncProgressOwnershipInvariant
          /\ AsyncCandidateServiceTombstoneLifecycleInvariant)

THEOREM AsyncLiveProvidesAdequateLeaderTargetProofInvariants ==
  \A initialContext:
    AdequateLeaderTargetProofInvariantsProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysProgressOwnershipInvariant,
   AsyncSpecAlwaysCandidateServiceTombstoneLifecycle,
   AsyncLiveSpecProjectsAsyncSpec, PTL
   DEF AdequateLeaderTargetProofInvariantsProperty

\* The four semantic providers compose one lexicographic step.  A
\* same-subject result must lower the occurrence component; a subject switch
\* must retire the named owner and lower the global frozen-owner component.
\* Exact corridor exit appears only in the common terminal handoff.
THEOREM AdequateLeaderBaseSemanticProvidersSupplyProductiveEpisodeRankStep ==
  \A initialContext:
    /\ AdequateLeaderTargetOccurrenceRankServiceProperty(
         AsyncLiveSpecAt(initialContext))
    /\ AdequateLeaderTargetProducerTransportOccurrenceClosureProperty(
         AsyncLiveSpecAt(initialContext))
    /\ AdequateLeaderTargetProducerTransportClosureProperty(
         AsyncLiveSpecAt(initialContext))
      => AdequateLeaderTargetProductiveEpisodeRankStepProperty(
           AsyncLiveSpecAt(initialContext))
BY AdequateLeaderOccurrenceAndProducerClosureProvideComposedRankDescent,
   AdequateLeaderComposedRankDescentClosesOccurrenceService,
   AdequateLeaderOccurrenceAndProducerClosureProvideSubjectSwitchCarry,
   AdequateLeaderOwnerIndexedServiceProvidesSubjectSwitchBudgetDescent,
   AdequateLeaderSubjectSwitchBudgetDescentClosesNamedOwnerEpisode,
   AdequateLeaderProtocolOccurrenceFrontierHasMinimalOwnerEpisode,
   AdequateLeaderOffSubjectReentryCarriesRetiredOwnerAndBudget,
   PTL, IsaT(2400)
   DEF AdequateLeaderTargetProductiveEpisodeRankStepProperty,
       AdequateLeaderTargetProductiveEpisodeRankFrontier,
       AdequateLeaderTargetProductiveEpisodeStrictDescentGoal,
       AdequateLeaderTargetProductiveEpisodeRankOrdering,
       AdequateLeaderTargetOccurrenceRankServiceExitGoal,
       AdequateLeaderTargetOccurrenceRankOwnerServiceExitGoal,
       AdequateLeaderTargetOccurrenceDecisionGoal,
       AdequateLeaderTargetOccurrenceStrictlyLowerGoal,
       AdequateLeaderTargetOffSubjectOccurrenceDrainGoal,
       AdequateLeaderTargetFrozenCorridorTerminalGoal,
       AdequateLeaderTargetAnyCorridorExitHandoff,
       SetLessThan, LexPairOrdering, OpToRel

AdequateLeaderFixedTargetCorridorConvergenceProperty(specification) ==
  specification
    => \A target \in ValidatorIds,
          leaderContext \in ContextRecords,
          leader \in ValidatorIds,
          leaderView \in Views,
          subject \in Subjects:
         AdequateLeaderTargetProductiveSubjectOpenFrontier(
           target, leaderContext, leader, leaderView, subject)
           ~> AdequateLeaderTargetFrozenCorridorTerminalGoal(
                target, leaderContext, leader, leaderView)

AdequateLeaderLocalFixedCorridorKernelProperty(specification) ==
  /\ AdequateLeaderTargetProofInvariantsProperty(specification)
  /\ AdequateLeaderTargetProducerTransportClosureProperty(specification)
  /\ AdequateLeaderTargetProductiveEpisodeRankStepProperty(specification)

THEOREM AdequateLeaderLocalFixedCorridorKernelSuppliesConvergence ==
  \A specification:
    AdequateLeaderLocalFixedCorridorKernelProperty(specification)
      => AdequateLeaderFixedTargetCorridorConvergenceProperty(specification)
BY AdequateLeaderProductiveEpisodeRankStepClosesFrozenCorridor,
   AdequateLeaderProtocolOccurrenceFrontierHasMinimalOwnerEpisode,
   PTL, IsaT(1200)
   DEF AdequateLeaderLocalFixedCorridorKernelProperty,
       AdequateLeaderTargetProofInvariantsProperty,
       AdequateLeaderTargetProducerTransportClosureProperty,
       AdequateLeaderTargetProductiveEpisodeClosureProperty,
       AdequateLeaderFixedTargetCorridorConvergenceProperty,
       AdequateLeaderTargetProductiveSubjectOpenFrontier,
       AdequateLeaderTargetOpenFrontier,
       AdequateLeaderTargetOccurrenceFrontierRankSet,
       AdequateLeaderTargetProductiveEpisodeRankFrontier,
       AdequateLeaderTargetFrozenCorridorTerminalGoal

\* Generic/local release surface.  It is intentionally per target and uses
\* no `ResponsiveNodesDecide`, joined, Apply, application, successor-height,
\* or one-height temporal-closure premise.  Indexed historical recovery may
\* instantiate `specification` with its frozen one-height Async behavior.
AdequateLeaderLocalSemanticKernelProperty(specification) ==
  /\ AdequateLeaderViewReachCompositionProperty(specification)
  /\ AdequateLeaderTargetCorridorEntryProperty(specification)
  /\ AdequateLeaderFixedTargetCorridorConvergenceProperty(specification)

THEOREM AdequateLeaderLocalSemanticKernelSuppliesTargetDecisionConvergence ==
  \A specification:
    AdequateLeaderLocalSemanticKernelProperty(specification)
      => AdequateLeaderLocalTargetDecisionConvergenceProperty(specification)
BY PTL
   DEF AdequateLeaderLocalSemanticKernelProperty,
       AdequateLeaderViewReachCompositionProperty,
       AdequateLeaderTargetCorridorEntryProperty,
       AdequateLeaderFixedTargetCorridorConvergenceProperty,
       AdequateLeaderLocalTargetDecisionConvergenceProperty,
       AdequateLeaderLocalTargetDecisionSource,
       AdequateLeaderTargetDecisionSource,
       AdequateLeaderTargetProductiveSubjectOpenFrontier,
       AdequateLeaderTargetFrozenCorridorTerminalGoal,
       AdequateLeaderTargetAnyCorridorExitHandoff

THEOREM AdequateLeaderLocalConvergenceSuppliesReachedViewConvergence ==
  \A specification:
    AdequateLeaderLocalTargetDecisionConvergenceProperty(specification)
      => AdequateLeaderTargetDecisionConvergenceProperty(specification)
BY PTL
   DEF AdequateLeaderLocalTargetDecisionConvergenceProperty,
       AdequateLeaderTargetDecisionConvergenceProperty,
       AdequateLeaderLocalTargetDecisionSource,
       AdequateLeaderTargetDecisionSource

THEOREM AdequateLeaderSemanticCompositionSuppliesLocalTargetConvergence ==
  \A initialContext:
    AdequateLeaderSemanticCompositionProperty(
      AsyncLiveSpecAt(initialContext))
      => AdequateLeaderLocalTargetDecisionConvergenceProperty(
           AsyncLiveSpecAt(initialContext))
BY AdequateLeaderBaseSemanticProvidersSupplyProductiveEpisodeRankStep,
   AsyncLiveProvidesAdequateLeaderTargetProofInvariants,
   AdequateLeaderLocalFixedCorridorKernelSuppliesConvergence,
   AdequateLeaderLocalSemanticKernelSuppliesTargetDecisionConvergence
   DEF AdequateLeaderSemanticCompositionProperty,
       AdequateLeaderTargetSemanticCompositionProperty,
       AdequateLeaderLocalFixedCorridorKernelProperty,
       AdequateLeaderLocalSemanticKernelProperty,
       AdequateLeaderFixedTargetCorridorConvergenceProperty

THEOREM AdequateLeaderTargetSemanticCompositionSuppliesTargetConvergence ==
  \A initialContext:
    AdequateLeaderSemanticCompositionProperty(
      AsyncLiveSpecAt(initialContext))
      => AdequateLeaderTargetDecisionConvergenceProperty(
           AsyncLiveSpecAt(initialContext))
BY AdequateLeaderSemanticCompositionSuppliesLocalTargetConvergence,
   AdequateLeaderLocalConvergenceSuppliesReachedViewConvergence
   DEF AdequateLeaderSemanticCompositionProperty,
       AdequateLeaderTargetSemanticCompositionProperty

THEOREM AdequateLeaderLocalTargetConvergenceSuppliesDecisionConvergence ==
  \A initialContext:
    AdequateLeaderLocalTargetDecisionConvergenceProperty(
      AsyncLiveSpecAt(initialContext))
      => ResponsiveDecisionConvergenceProperty(
           AsyncLiveSpecAt(initialContext))
BY AdequateLeaderDecisionPrefixAtIsStable,
   AdequateLeaderAsyncBracketStepPreservesTargetDecision,
   FrozenContextFullAdequateLeaderDecisionPrefixImpliesResponsiveDecide,
   FrozenContextFixesResponsiveVoters,
   NatInduction, PTL, IsaT(1800)
   DEF AdequateLeaderLocalTargetDecisionConvergenceProperty,
       AdequateLeaderLocalTargetDecisionSource,
       AdequateLeaderDecisionPrefixAt,
       ResponsiveDecisionConvergenceProperty,
       ResponsiveNodesDecide,
       AsyncVotersAt, ValidatorIds

THEOREM ExactAdequateLeaderSubkernelsReduceToServiceKernel ==
  \A initialContext:
    /\ AdequateLeaderExactResidualKernelProperty(
         AsyncLiveSpecAt(initialContext))
    /\ AdequateLeaderSemanticCompositionProperty(
         AsyncLiveSpecAt(initialContext))
    => AdequateLeaderServiceKernelProperty(
         AsyncLiveSpecAt(initialContext))
BY AdequateLeaderTargetSemanticCompositionSuppliesTargetConvergence,
   AdequateLeaderSemanticCompositionSuppliesLocalTargetConvergence,
   AdequateLeaderLocalTargetConvergenceSuppliesDecisionConvergence,
   PTL
   DEF AdequateLeaderSemanticCompositionProperty,
       AdequateLeaderViewReachCompositionProperty,
       AdequateLeaderServiceKernelProperty,
       ResponsiveDecisionConvergenceProperty

=============================================================================
