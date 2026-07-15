---- MODULE SumeragiV2SafetyLemmas ----
EXTENDS SumeragiV2SafetyDefinitions, SumeragiV2QuorumProofs

(***************************************************************************
Compositional safety lemmas for the production reducer.

These theorems are deliberately state-independent.  They prove the algebraic
consequences of the reducer's durable intent, certificate provenance, body,
lock, and timeout invariants.  SumeragiV2Proofs separately records the still
required action-by-action proof that every executable transition establishes
and preserves those antecedents.
***************************************************************************)

THEOREM DurableVoteAppendPreservesUniqueness ==
  \A votes, vote:
    (HonestVoteUnique(votes) /\ CanAppendVote(votes, vote))
      => HonestVoteUnique(votes \cup {vote})
BY SMT DEF HonestVoteUnique, CanAppendVote, SameVoteSlot

THEOREM DurableTimeoutAppendPreservesUniqueness ==
  \A votes, vote:
    (HonestTimeoutUnique(votes) /\ CanAppendTimeout(votes, vote))
      => HonestTimeoutUnique(votes \cup {vote})
BY SMT
   DEF HonestTimeoutUnique, CanAppendTimeout,
       SameTimeoutSlot, SameTimeoutContent

(***************************************************************************
Certificate provenance and same-view uniqueness.
***************************************************************************)

THEOREM SameViewCertificateUniqueness ==
  \A epoch \in Epochs:
    \A intents, left, right:
      (/\ QuorumConfiguration
       /\ HonestVoteUnique(intents)
       /\ CertificateBackedBy(epoch, left, intents)
       /\ CertificateBackedBy(epoch, right, intents)
       /\ SameCertificateSlot(left, right))
      => left.subject = right.subject
PROOF
  <1>1. ASSUME NEW epoch \in Epochs,
              NEW intents,
              NEW left,
              NEW right,
              QuorumConfiguration,
              HonestVoteUnique(intents),
              CertificateBackedBy(epoch, left, intents),
              CertificateBackedBy(epoch, right, intents),
              SameCertificateSlot(left, right)
         PROVE left.subject = right.subject
    <2>1. DualQuorumIntersectionHasHonest
      BY <1>1, DualQuorumHonestIntersection
    <2>2. /\ left.signers \in SUBSET VotingRoster(epoch)
          /\ right.signers \in SUBSET VotingRoster(epoch)
          /\ DualQuorum(epoch, left.signers)
          /\ DualQuorum(epoch, right.signers)
      BY <1>1 DEF CertificateBackedBy, DualQuorum, CountQuorum
    <2>3. (left.signers \cap right.signers \cap Honest) # {}
      BY <2>1, <2>2
         DEF CertificateBackedBy, DualQuorumIntersectionHasHonest
    <2>4. PICK signer \in left.signers \cap right.signers \cap Honest: TRUE
      BY <2>3
    <2>5. PICK leftVote \in intents:
             VoteBacksCertificate(leftVote, left, signer)
      BY <1>1, <2>4 DEF CertificateBackedBy
    <2>6. PICK rightVote \in intents:
             VoteBacksCertificate(rightVote, right, signer)
      BY <1>1, <2>4 DEF CertificateBackedBy
    <2>7. /\ leftVote.signer \in Honest
          /\ SameVoteSlot(leftVote, rightVote)
      BY <1>1, <2>4, <2>5, <2>6
         DEF VoteBacksCertificate, SameCertificateSlot, SameVoteSlot
    <2>8. leftVote.subject = rightVote.subject
      BY <1>1, <2>5, <2>6, <2>7 DEF HonestVoteUnique
    <2> QED BY <2>5, <2>6, <2>8 DEF VoteBacksCertificate
  <1> QED BY <1>1

(***************************************************************************
External validity and decided-body availability.  Body retention is expressed
as a durable-store membership fact for every honest intent.  An honest member
of any dual quorum therefore supplies both deterministic validity and a body.
***************************************************************************)

THEOREM BackedCertificateIsValidAndAvailable ==
  \A epoch \in Epochs:
    \A intents, qc, durable, validSubjects:
      (/\ QuorumConfiguration
       /\ CertificateBackedBy(epoch, qc, intents)
       /\ HonestIntentSound(intents, durable, validSubjects))
      => CertificateValidityAndAvailability(qc, durable, validSubjects)
PROOF
  <1>1. ASSUME NEW epoch \in Epochs,
              NEW intents,
              NEW qc,
              NEW durable,
              NEW validSubjects,
              QuorumConfiguration,
              CertificateBackedBy(epoch, qc, intents),
              HonestIntentSound(intents, durable, validSubjects)
         PROVE CertificateValidityAndAvailability(
                 qc, durable, validSubjects)
    <2>1. DualQuorumIntersectionHasHonest
      BY <1>1, DualQuorumHonestIntersection
    <2>2. /\ qc.signers \in SUBSET VotingRoster(epoch)
          /\ DualQuorum(epoch, qc.signers)
      BY <1>1 DEF CertificateBackedBy, DualQuorum, CountQuorum
    <2>3. (qc.signers \cap qc.signers \cap Honest) # {}
      BY <2>1, <2>2
         DEF CertificateBackedBy, DualQuorumIntersectionHasHonest
    <2>4. PICK signer \in qc.signers \cap Honest: TRUE
      BY <2>3, Isa
    <2>5. PICK vote \in intents:
             VoteBacksCertificate(vote, qc, signer)
      BY <1>1, <2>4 DEF CertificateBackedBy
    <2>6. /\ vote.subject \in validSubjects
          /\ BodyHeldBy(durable, vote.signer,
                        vote.context, vote.view, vote.subject)
      BY <1>1, <2>4, <2>5 DEF HonestIntentSound, VoteBacksCertificate
    <2> QED BY <2>4, <2>5, <2>6
       DEF CertificateValidityAndAvailability, VoteBacksCertificate
  <1> QED BY <1>1

(***************************************************************************
Lock monotonicity for the only two production lock updates: atomic
PrepareQC+Commit persistence and installation of a TC-selected high QC.
***************************************************************************)

THEOREM CommitPersistenceAdvancesLockMonotonically ==
  \A oldLock, qc:
    (/\ oldLock.rank \in Int
     /\ qc.view \in Int
     /\ CommitLockAllowed(oldLock, qc))
      => LockMonotone(oldLock, CommitLockResult(qc))
PROOF
  <1>1. ASSUME NEW oldLock,
              NEW qc,
              oldLock.rank \in Int,
              qc.view \in Int,
              CommitLockAllowed(oldLock, qc)
         PROVE LockMonotone(oldLock, CommitLockResult(qc))
    <2>1. /\ CommitLockResult(qc).rank = qc.view
          /\ CommitLockResult(qc).subject = qc.subject
      BY DEF CommitLockResult, LockValue
    <2>2. CommitLockResult(qc).rank >= oldLock.rank
      BY <1>1, <2>1 DEF CommitLockAllowed
    <2>3. CommitLockResult(qc).subject # oldLock.subject
              => CommitLockResult(qc).rank > oldLock.rank
      BY <1>1, <2>1, SMT DEF CommitLockAllowed
    <2> QED BY <2>2, <2>3 DEF LockMonotone
  <1> QED BY <1>1

THEOREM TimeoutInstallationAdvancesLockMonotonically ==
  \A oldLock, selectedRank, selectedSubject:
    (/\ oldLock.rank \in Int
     /\ selectedRank \in Int)
    => LockMonotone(
         oldLock, InstallHighLock(oldLock, selectedRank, selectedSubject))
BY SMT DEF InstallHighLock, LockValue, LockMonotone

THEOREM MonotoneLockUpdatesCompose ==
  \A first, second, third:
    (/\ first.rank \in Int
     /\ second.rank \in Int
     /\ third.rank \in Int
     /\ LockMonotone(first, second)
     /\ LockMonotone(second, third))
      => LockMonotone(first, third)
PROOF
  <1>1. ASSUME NEW first,
              NEW second,
              NEW third,
              first.rank \in Int,
              second.rank \in Int,
              third.rank \in Int,
              LockMonotone(first, second),
              LockMonotone(second, third)
         PROVE LockMonotone(first, third)
    <2>1. third.rank >= first.rank
      BY <1>1, SMT DEF LockMonotone
    <2>2. third.subject # first.subject
              => (second.subject # first.subject
                    \/ third.subject # second.subject)
      BY Zenon
    <2>3. second.subject # first.subject
              => third.rank > first.rank
      BY <1>1, SMT DEF LockMonotone
    <2>4. third.subject # second.subject
              => third.rank > first.rank
      BY <1>1, SMT DEF LockMonotone
    <2>5. third.subject # first.subject
              => third.rank > first.rank
      BY <2>2, <2>3, <2>4, Zenon
    <2> QED BY <2>1, <2>5 DEF LockMonotone
  <1> QED BY <1>1

(***************************************************************************
Grouped timeout protection.  TimeoutIntentProtectsCommits is the durable
fence invariant: if one honest signer has both an old Commit intent and a TC
timeout vote, that timeout reports a PrepareQC no lower than the Commit lock,
and reports the same subject when ranks are equal.  The theorem combines that
local fact with dual-quorum honest intersection and TC maximum selection.
***************************************************************************)

THEOREM GroupedTimeoutProtectsCommitQuorum ==
  \A epoch \in Epochs:
    \A tc, commitIntentSet, protectedView, subject:
      (/\ QuorumConfiguration
       /\ TimeoutProtectionKernel(
            epoch, tc, commitIntentSet, protectedView, subject))
      => TCProtectsViewSubject(tc, protectedView, subject)
PROOF
  <1>1. ASSUME NEW epoch \in Epochs,
              NEW tc,
              NEW commitIntentSet,
              NEW protectedView,
              NEW subject,
              QuorumConfiguration,
              TimeoutProtectionKernel(
                epoch, tc, commitIntentSet, protectedView, subject)
         PROVE TCProtectsViewSubject(tc, protectedView, subject)
    <2> DEFINE CommitSigners ==
           CommitSignerSet(
             commitIntentSet, tc.context, protectedView, subject)
    <2> DEFINE TimeoutSigners == TimeoutSignerSet(tc.votes)
    <2>1. /\ DualQuorum(epoch, CommitSigners)
          /\ DualQuorum(epoch, TimeoutSigners)
      BY <1>1 DEF TimeoutProtectionKernel, CommitSigners, TimeoutSigners
    <2>2. DualQuorumIntersectionHasHonest
      BY <1>1, DualQuorumHonestIntersection
    <2>3. /\ CommitSigners \in SUBSET VotingRoster(epoch)
          /\ TimeoutSigners \in SUBSET VotingRoster(epoch)
      BY <2>1 DEF DualQuorum, CountQuorum
    <2>4. (CommitSigners \cap TimeoutSigners \cap Honest) # {}
      BY <2>1, <2>2, <2>3
         DEF DualQuorumIntersectionHasHonest
    <2>5. PICK signer \in CommitSigners \cap TimeoutSigners \cap Honest:
             TRUE
      BY <2>4
    <2>6. PICK commitVote \in commitIntentSet:
             /\ commitVote.signer = signer
             /\ commitVote.context = tc.context
             /\ commitVote.view = protectedView
             /\ commitVote.phase = "Commit"
             /\ commitVote.subject = subject
      BY <2>5 DEF CommitSigners, CommitSignerSet
    <2>7. PICK timeoutVote \in tc.votes:
             timeoutVote.signer = signer
      BY <2>5 DEF TimeoutSigners, TimeoutSignerSet
    <2>8. /\ timeoutVote.context = tc.context
          /\ timeoutVote.view = tc.view
      BY <1>1, <2>7
         DEF TimeoutProtectionKernel, TimeoutVotesBindCertificate
    <2>9. /\ timeoutVote.highRank >= protectedView
          /\ (timeoutVote.highRank = protectedView
                => timeoutVote.highSubject = subject)
      BY <1>1, <2>5, <2>6, <2>7, <2>8
         DEF TimeoutProtectionKernel, TimeoutIntentProtectsCommits,
             TimeoutVoteProtectsCommitSet
    <2>10. /\ TcHighRank(tc) >= timeoutVote.highRank
          /\ (TcHighRank(tc) = timeoutVote.highRank
                => TcHighSubject(tc) = timeoutVote.highSubject)
      BY <1>1, <2>7
         DEF TimeoutProtectionKernel, TCMaximumProtectsReports
    <2>11. /\ protectedView \in Int
           /\ timeoutVote.highRank \in Int
           /\ TcHighRank(tc) \in Int
      BY <1>1, <2>7
         DEF TimeoutProtectionKernel, TimeoutRanksTyped
    <2>12. TcHighRank(tc) >= protectedView
      BY <2>9, <2>10, <2>11, SMT
    <2>13. TcHighRank(tc) = protectedView
               => /\ timeoutVote.highRank = protectedView
                  /\ timeoutVote.highSubject = subject
                  /\ TcHighSubject(tc) = timeoutVote.highSubject
      BY <2>9, <2>10, <2>11, SMT
    <2> QED BY <2>12, <2>13, SMT DEF TCProtectsViewSubject
  <1> QED BY <1>1

=============================================================================
