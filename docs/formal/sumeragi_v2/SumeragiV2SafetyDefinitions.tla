---- MODULE SumeragiV2SafetyDefinitions ----
EXTENDS SumeragiV2Core

(***************************************************************************
Executable safety vocabulary shared by TLC and TLAPS.  Keeping definitions
separate from the proof modules lets bounded models load the inductive
invariant without importing TLAPS-only libraries.
***************************************************************************)

SameVoteSlot(left, right) ==
  /\ left.context = right.context
  /\ left.view = right.view
  /\ left.phase = right.phase
  /\ left.signer = right.signer

HonestVoteUnique(votes) ==
  \A left, right \in votes:
    (left.signer \in Honest /\ SameVoteSlot(left, right))
      => left.subject = right.subject

CanAppendVote(votes, vote) ==
  vote.signer \notin Honest
    \/ \A prior \in votes:
         SameVoteSlot(prior, vote) => prior.subject = vote.subject

SameTimeoutSlot(left, right) ==
  /\ left.context = right.context
  /\ left.view = right.view
  /\ left.signer = right.signer

SameTimeoutContent(left, right) ==
  /\ left.highRank = right.highRank
  /\ left.highSubject = right.highSubject

HonestTimeoutUnique(votes) ==
  \A left, right \in votes:
    (left.signer \in Honest /\ SameTimeoutSlot(left, right))
      => SameTimeoutContent(left, right)

CanAppendTimeout(votes, vote) ==
  vote.signer \notin Honest
    \/ \A prior \in votes:
         SameTimeoutSlot(prior, vote) => SameTimeoutContent(prior, vote)

CertificateBackedBy(epoch, qc, intents) ==
  /\ DualQuorum(epoch, qc.signers)
  /\ \A signer \in qc.signers \cap Honest:
       \E vote \in intents: VoteBacksCertificate(vote, qc, signer)

SameCertificateSlot(left, right) ==
  /\ left.context = right.context
  /\ left.view = right.view
  /\ left.phase = right.phase

HonestIntentSound(intents, durable, validSubjects) ==
  \A vote \in intents:
    vote.signer \in Honest
      => /\ vote.subject \in validSubjects
         /\ BodyHeldBy(durable, vote.signer,
                       vote.context, vote.view, vote.subject)

CertificateValidityAndAvailability(qc, durable, validSubjects) ==
  /\ qc.subject \in validSubjects
  /\ \E signer \in qc.signers \cap Honest:
       BodyHeldBy(durable, signer, qc.context, qc.view, qc.subject)

LockValue(rank, subject) == [rank |-> rank, subject |-> subject]

CommitLockAllowed(oldLock, qc) ==
  /\ qc.view >= oldLock.rank
  /\ (qc.view = oldLock.rank => qc.subject = oldLock.subject)

CommitLockResult(qc) == LockValue(qc.view, qc.subject)

InstallHighLock(oldLock, selectedRank, selectedSubject) ==
  IF selectedRank > oldLock.rank
  THEN LockValue(selectedRank, selectedSubject)
  ELSE oldLock

LockMonotone(oldLock, newLock) ==
  /\ newLock.rank >= oldLock.rank
  /\ (newLock.subject # oldLock.subject
        => newLock.rank > oldLock.rank)

CommitSignerSet(intents, certificateContext, protectedView, subject) ==
  {vote.signer:
    vote \in {candidate \in intents:
      /\ candidate.context = certificateContext
      /\ candidate.view = protectedView
      /\ candidate.phase = "Commit"
      /\ candidate.subject = subject}}

TimeoutIntentProtectsCommits(timeoutVotes, commitIntentSet) ==
  \A timeoutVote \in timeoutVotes:
    TimeoutVoteProtectsCommitSet(timeoutVote, commitIntentSet)

(***************************************************************************
The algebraic grouped-timeout kernel below predates Commit creation: every
intersecting honest timeout must itself report a high QC protecting the
candidate Commit.  Keep that strict premise separate from the executable
state invariant above, which also admits a later exact Commit authorized by a
durably installed TC.  Relating those cross-TC histories is an explicit
inductive obligation, not part of the already-proved quorum-set kernel.
***************************************************************************)
StrictTimeoutVoteProtectsCommitSet(timeoutVote, commitIntentSet) ==
  \A commitVote \in commitIntentSet:
    (/\ timeoutVote.signer \in Honest
     /\ commitVote.signer = timeoutVote.signer
     /\ commitVote.context = timeoutVote.context
     /\ commitVote.phase = "Commit"
     /\ commitVote.view <= timeoutVote.view)
    => TimeoutVoteStrictlyProtectsCommit(timeoutVote, commitVote)

StrictTimeoutIntentProtectsCommits(timeoutVotes, commitIntentSet) ==
  \A timeoutVote \in timeoutVotes:
    StrictTimeoutVoteProtectsCommitSet(timeoutVote, commitIntentSet)

TCMaximumProtectsReports(tc) ==
  \A timeoutVote \in tc.votes:
    /\ TcHighRank(tc) >= timeoutVote.highRank
    /\ (TcHighRank(tc) = timeoutVote.highRank
          => TcHighSubject(tc) = timeoutVote.highSubject)

TimeoutVotesBindCertificate(tc) ==
  \A timeoutVote \in tc.votes:
    /\ timeoutVote.context = tc.context
    /\ timeoutVote.view = tc.view

TimeoutRanksTyped(tc, protectedView) ==
  /\ protectedView \in Int
  /\ TcHighRank(tc) \in Int
  /\ \A timeoutVote \in tc.votes: timeoutVote.highRank \in Int

TimeoutProtectionKernel(epoch, tc, commitIntentSet,
                        protectedView, subject) ==
  LET commitSigners ==
        CommitSignerSet(commitIntentSet, tc.context, protectedView, subject)
      timeoutSigners == TimeoutSignerSet(tc.votes)
  IN /\ protectedView <= tc.view
     /\ DualQuorum(epoch, commitSigners)
     /\ DualQuorum(epoch, timeoutSigners)
     /\ StrictTimeoutIntentProtectsCommits(tc.votes, commitIntentSet)
     /\ TimeoutVotesDisjoint(tc.votes)
     /\ TimeoutVotesBindCertificate(tc)
     /\ TimeoutRanksTyped(tc, protectedView)
     /\ TCMaximumProtectsReports(tc)

TCProtectsViewSubject(tc, protectedView, subject) ==
  /\ TcHighRank(tc) >= protectedView
  /\ (TcHighRank(tc) = protectedView
        => TcHighSubject(tc) = subject)

=============================================================================
