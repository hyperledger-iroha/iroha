---- MODULE SumeragiV2ExactLeaderSemanticRanks ----
EXTENDS SumeragiV2ProgressWitnessFinalClosureProofs

(***************************************************************************
Neutral exact-leader semantic-rank vocabulary.

Locked-body reproposal is below rotating-leader progress in the release
dependency order, while adequate-leader service is above it.  The two leaves
nevertheless classify the same concrete scheduler candidates.  Keep that
classification in this operator-only lower module so neither proof leaf must
import the other and no temporal conclusion can flow backwards through the
shared vocabulary.

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

ExactLeaderSemanticRankCarrier == (1..5) \X (0..9)

ExactLeaderSemanticRankOrdering ==
  LexPairOrdering(
    OpToRel(<, Nat), OpToRel(<, Nat), 1..5, 0..9)

MatchingVoteSignRequest(candidate, phase) ==
  \E request \in signVotes:
    /\ request.node = candidate.node
    /\ request.vote.context = candidate.consumerContext
    /\ request.vote.view = candidate.view
    /\ request.vote.subject = candidate.subject
    /\ request.vote.phase = phase

(***************************************************************************
Replay-safe Proposal ownership.

Production rechecks every durable ProposalIntent against the current durable
lock before re-signing it.  A same-round lock for subject B supersedes an
earlier ProposalIntent for subject A; replay resumes B's durable Commit owner
and must not turn A into semantic Proposal progress.

The protected asynchronous model deliberately retains the larger durable WAL
projection in `RestartProposalIntents`.  Keep that abstraction conservative by
excluding an unsafe replayed SignProposal from the exact-leader rank.  The
predicate below is the exact formal analogue of production's shared
`proposal_is_safe_for_lock`: the same locked subject remains safe, while a
different subject requires a strictly higher PrepareQC for that subject.
***************************************************************************)

DurableProposalSafeForLock(node, proposal) ==
  \/ lockRank[node] = NoRank
  \/ /\ proposal.view >= lockRank[node]
     /\ proposal.subject = lockSubject[node]
  \/ /\ proposal.view >= lockRank[node]
     /\ proposal.timeoutCertificate # NoTimeoutCertificate
     /\ proposal.highestPrepareQc # NoPrepareQC
     /\ proposal.timeoutCertificate.highestPrepareQc =
          proposal.highestPrepareQc
     /\ proposal.highestPrepareQc.phase = "Prepare"
     /\ proposal.highestPrepareQc.context = proposal.context
     /\ proposal.highestPrepareQc.height = proposal.height
     /\ proposal.highestPrepareQc.view > lockRank[node]
     /\ proposal.highestPrepareQc.subject = proposal.subject

ProposalSignIntentsAt(candidate) ==
  {proposal \in proposalIntents:
    /\ proposal.proposer = candidate.node
    /\ proposal.context = candidate.consumerContext
    /\ proposal.height = candidate.height
    /\ proposal.view = candidate.view
    /\ proposal.subject = candidate.subject}

SafeProposalSignIntentAt(candidate) ==
  LET matching == ProposalSignIntentsAt(candidate)
  IN /\ matching # {}
     /\ \A proposal \in matching:
          DurableProposalSafeForLock(candidate.node, proposal)

ExactLeaderViewChangeRank(candidate, rank) ==
  \/ /\ candidate.kind = "PersistTimeout"
        /\ rank = ViewChangeSemanticRank(7)
  \/ /\ candidate.kind = "SignTimeout"
        /\ rank = ViewChangeSemanticRank(6)
  \/ /\ candidate.kind = "DeliverTimeout"
        /\ candidate.item.kind = "TimeoutVote"
        /\ rank = ViewChangeSemanticRank(5)
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
        /\ SafeProposalSignIntentAt(candidate)
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

=============================================================================
