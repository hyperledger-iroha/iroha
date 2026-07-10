---- MODULE SumeragiV2AgreementLemmas ----
EXTENDS SumeragiV2Inductive, SumeragiV2SafetyLemmas, NaturalsInduction

(***************************************************************************
Cross-view safety is exposed as a descending certificate lineage.  A later
PrepareQC that conflicts with an older CommitQC intersects the Commit quorum
in an honest validator.  That validator's durable Prepare lineage must carry
a strictly higher-than-Commit but strictly lower-than-later PrepareQC for the
new subject.  Repeating this descent rules out a lowest conflicting view.
***************************************************************************)

THEOREM ConflictingLaterPrepareCarriesIntermediateQc ==
  QuorumConfiguration
    /\ CertificatesBackedByIntents
    /\ IntentPhasesCorrect
    /\ PrepareLineageSound
    => \A committed \in commitQCs, later \in prepareQCs:
         (/\ committed.context = later.context
          /\ committed.view < later.view
          /\ committed.subject # later.subject)
         => \E carried \in prepareQCs:
              /\ carried.context = committed.context
              /\ carried.phase = "Prepare"
              /\ committed.view < carried.view
              /\ carried.view < later.view
              /\ carried.subject = later.subject
PROOF
  <1>1. ASSUME QuorumConfiguration,
              CertificatesBackedByIntents,
              IntentPhasesCorrect,
              PrepareLineageSound,
              NEW committed \in commitQCs,
              NEW later \in prepareQCs,
              committed.context = later.context,
              committed.view < later.view,
              committed.subject # later.subject
         PROVE \E carried \in prepareQCs:
                 /\ carried.context = committed.context
                 /\ carried.phase = "Prepare"
                 /\ committed.view < carried.view
                 /\ carried.view < later.view
                 /\ carried.subject = later.subject
    <2>1. /\ CertificateBackedBy(committed.context.epoch,
                                committed, commitIntents)
          /\ CertificateBackedBy(later.context.epoch,
                                later, prepareIntents)
      BY <1>1 DEF CertificatesBackedByIntents
    <2>2. committed.context.epoch = later.context.epoch
      BY <1>1
    <2>3. DualQuorumIntersectionHasHonest
      BY <1>1, DualQuorumHonestIntersection
    <2>4. /\ committed.context.epoch \in Epochs
          /\ committed.signers
               \in SUBSET VotingRoster(committed.context.epoch)
          /\ later.signers
               \in SUBSET VotingRoster(committed.context.epoch)
          /\ DualQuorum(committed.context.epoch, committed.signers)
          /\ DualQuorum(committed.context.epoch, later.signers)
      BY <1>1, <2>1, <2>2
         DEF CertificateBackedBy, DualQuorum, CountQuorum
    <2>5. (committed.signers \cap later.signers \cap Honest) # {}
      BY <2>3, <2>4 DEF DualQuorumIntersectionHasHonest
    <2>6. PICK signer \in
                 committed.signers \cap later.signers \cap Honest: TRUE
      BY <2>5
    <2>7. PICK commitVote \in commitIntents:
             VoteBacksCertificate(commitVote, committed, signer)
      BY <2>1, <2>6 DEF CertificateBackedBy
    <2>8. PICK prepareVote \in prepareIntents:
             VoteBacksCertificate(prepareVote, later, signer)
      BY <2>1, <2>2, <2>6 DEF CertificateBackedBy
    <2>9. /\ commitVote.context = committed.context
          /\ commitVote.view = committed.view
          /\ commitVote.phase = committed.phase
          /\ commitVote.subject = committed.subject
          /\ commitVote.signer = signer
          /\ prepareVote.context = later.context
          /\ prepareVote.view = later.view
          /\ prepareVote.phase = later.phase
          /\ prepareVote.subject = later.subject
          /\ prepareVote.signer = signer
      BY <2>7, <2>8 DEF VoteBacksCertificate
    <2>10. signer \in Honest
      BY <2>6, Isa
    <2>11. /\ prepareVote.signer \in Honest
          /\ commitVote.signer = prepareVote.signer
          /\ commitVote.context = prepareVote.context
          /\ commitVote.phase = "Commit"
          /\ commitVote.view < prepareVote.view
          /\ commitVote.subject # prepareVote.subject
      BY <1>1, <2>7, <2>8, <2>9, <2>10
         DEF VoteBacksCertificate, IntentPhasesCorrect
    <2>12. PrepareCarriesHigherSafeQc(prepareVote)
      BY <1>1, <2>8, <2>10 DEF PrepareLineageSound,
                                    VoteBacksCertificate
    <2>13. PICK carried \in prepareQCs:
              /\ carried.context = prepareVote.context
              /\ carried.phase = "Prepare"
              /\ commitVote.view < carried.view
              /\ carried.view < prepareVote.view
              /\ carried.subject = prepareVote.subject
      BY <2>11, <2>12 DEF PrepareCarriesHigherSafeQc
    <2>14. carried.context = committed.context
      BY <1>1, <2>9, <2>13, Zenon
    <2>15. committed.view < carried.view
      BY <2>9, <2>13, SMT
    <2>16. carried.view < later.view
      BY <2>9, <2>13, SMT
    <2>17. carried.subject = later.subject
      BY <2>9, <2>13, SMT
    <2>18. /\ carried.context = committed.context
           /\ carried.phase = "Prepare"
           /\ committed.view < carried.view
           /\ carried.view < later.view
           /\ carried.subject = later.subject
      BY <2>13, <2>14, <2>15, <2>16, <2>17
    <2> QED BY <2>13, <2>18
  <1> QED BY <1>1

(***************************************************************************
The descent lemma cannot repeat forever because views are natural numbers.
Choosing the least conflicting later PrepareQC yields an even earlier
conflicting PrepareQC, a contradiction.  This is the well-founded step that
turns local safe-unlock lineage into cross-view protection.
***************************************************************************)

ConflictingPrepareAt(committed, roundView) ==
  \E later \in prepareQCs:
    /\ later.context = committed.context
    /\ later.view = roundView
    /\ committed.view < later.view
    /\ committed.subject # later.subject

THEOREM CommitExcludesConflictingLaterPrepare ==
  QuorumConfiguration
    /\ CertificatesBackedByIntents
    /\ IntentPhasesCorrect
    /\ PrepareLineageSound
    => \A committed \in commitQCs, later \in prepareQCs:
         (committed.context = later.context
           /\ committed.view < later.view)
           => committed.subject = later.subject
PROOF
  <1>1. ASSUME QuorumConfiguration,
              CertificatesBackedByIntents,
              IntentPhasesCorrect,
              PrepareLineageSound,
              NEW committed \in commitQCs,
              NEW later \in prepareQCs,
              committed.context = later.context,
              committed.view < later.view
         PROVE committed.subject = later.subject
    <2>1. SUFFICES ASSUME committed.subject # later.subject
                     PROVE FALSE
      BY <1>1, Zenon
    <2>2. ConflictingPrepareAt(committed, later.view)
      BY <1>1, <2>1 DEF ConflictingPrepareAt
    <2>3. later.view \in Nat
      BY <1>1 DEF CertificatesBackedByIntents, HistoricalQcValid,
                    Views
    <2>4. \E least \in Nat:
             /\ ConflictingPrepareAt(committed, least)
             /\ \A prior \in 0..(least - 1):
                  ~ConflictingPrepareAt(committed, prior)
      BY <2>2, <2>3, SmallestNatural
    <2>5. PICK least \in Nat:
             /\ ConflictingPrepareAt(committed, least)
             /\ \A prior \in 0..(least - 1):
                  ~ConflictingPrepareAt(committed, prior)
      BY <2>4
    <2>6. PICK leastLater \in prepareQCs:
             /\ leastLater.context = committed.context
             /\ leastLater.view = least
             /\ committed.view < leastLater.view
             /\ committed.subject # leastLater.subject
      BY <2>5 DEF ConflictingPrepareAt
    <2>7. PICK carried \in prepareQCs:
             /\ carried.context = committed.context
             /\ carried.phase = "Prepare"
             /\ committed.view < carried.view
             /\ carried.view < leastLater.view
             /\ carried.subject = leastLater.subject
      BY <1>1, <2>6, ConflictingLaterPrepareCarriesIntermediateQc
    <2>8. carried.view \in Nat
      BY <1>1, <2>7 DEF CertificatesBackedByIntents,
                             HistoricalQcValid, Views
    <2>9. carried.view \in 0..(least - 1)
      BY <2>5, <2>6, <2>7, <2>8, SMT
    <2>10. ConflictingPrepareAt(committed, carried.view)
      BY <2>6, <2>7 DEF ConflictingPrepareAt
    <2> QED BY <2>5, <2>9, <2>10
  <1> QED BY <1>1

(***************************************************************************
Every CommitQC contains an honest signer.  That signer's durable Commit
intent names the PrepareQC atomically persisted with its lock, so a CommitQC
always has a matching PrepareQC at the same context, view, and subject.
***************************************************************************)

THEOREM CommitCertificateHasPrepareCertificate ==
  QuorumConfiguration
    /\ CertificatesBackedByIntents
    /\ HonestCommitIntentPrepared
    => \A committed \in commitQCs:
         \E prepared \in prepareQCs:
           /\ prepared.context = committed.context
           /\ prepared.view = committed.view
           /\ prepared.phase = "Prepare"
           /\ prepared.subject = committed.subject
PROOF
  <1>1. ASSUME QuorumConfiguration,
              CertificatesBackedByIntents,
              HonestCommitIntentPrepared,
              NEW committed \in commitQCs
         PROVE \E prepared \in prepareQCs:
                 /\ prepared.context = committed.context
                 /\ prepared.view = committed.view
                 /\ prepared.phase = "Prepare"
                 /\ prepared.subject = committed.subject
    <2>1. /\ committed.context.epoch \in Epochs
          /\ DualQuorum(committed.context.epoch, committed.signers)
          /\ CertificateBackedBy(committed.context.epoch, committed,
                                 commitIntents)
      BY <1>1 DEF CertificatesBackedByIntents, HistoricalQcValid
    <2>2. DualQuorumIntersectionHasHonest
      BY <1>1, DualQuorumHonestIntersection
    <2>3. /\ committed.signers
               \in SUBSET VotingRoster(committed.context.epoch)
          /\ (committed.signers \cap committed.signers \cap Honest) # {}
      BY <2>1, <2>2
         DEF CertificateBackedBy, DualQuorum, CountQuorum,
             DualQuorumIntersectionHasHonest
    <2>4. PICK signer \in committed.signers \cap Honest: TRUE
      BY <2>3, Isa
    <2>5. PICK commitVote \in commitIntents:
             VoteBacksCertificate(commitVote, committed, signer)
      BY <2>1, <2>4 DEF CertificateBackedBy
    <2>6. commitVote.signer \in Honest
      BY <2>4, <2>5 DEF VoteBacksCertificate
    <2>7. PICK prepared \in prepareQCs:
             /\ prepared.context = commitVote.context
             /\ prepared.view = commitVote.view
             /\ prepared.phase = "Prepare"
             /\ prepared.subject = commitVote.subject
      BY <1>1, <2>5, <2>6 DEF HonestCommitIntentPrepared
    <2>8. /\ prepared.context = committed.context
          /\ prepared.view = committed.view
          /\ prepared.phase = "Prepare"
          /\ prepared.subject = committed.subject
      BY <2>5, <2>7 DEF VoteBacksCertificate
    <2> QED BY <2>7, <2>8
  <1> QED BY <1>1

(***************************************************************************
Same-view CommitQCs are unique by quorum intersection and durable sign-once.
Different-view CommitQCs reduce to the matching later PrepareQC and the
well-founded exclusion theorem above.
***************************************************************************)

THEOREM CommitCertificateAgreement ==
  QuorumConfiguration
    /\ CertificatesBackedByIntents
    /\ IntentPhasesCorrect
    /\ HonestVoteUnique(commitIntents)
    /\ PrepareLineageSound
    /\ HonestCommitIntentPrepared
    => \A left, right \in commitQCs:
         left.context = right.context => left.subject = right.subject
PROOF
  <1>1. ASSUME QuorumConfiguration,
              CertificatesBackedByIntents,
              IntentPhasesCorrect,
              HonestVoteUnique(commitIntents),
              PrepareLineageSound,
              HonestCommitIntentPrepared,
              NEW left \in commitQCs,
              NEW right \in commitQCs,
              left.context = right.context
         PROVE left.subject = right.subject
    <2>1. CASE left.view = right.view
      <3>1. SameCertificateSlot(left, right)
        BY <1>1, <2>1 DEF CertificatesBackedByIntents,
                          HistoricalQcValid, IntentPhasesCorrect,
                          CertificateBackedBy, VoteBacksCertificate,
                          SameCertificateSlot
      <3> QED
        BY <1>1, <3>1, SameViewCertificateUniqueness
           DEF CertificatesBackedByIntents
    <2>2. CASE left.view < right.view
      <3>1. PICK prepared \in prepareQCs:
               /\ prepared.context = right.context
               /\ prepared.view = right.view
               /\ prepared.phase = "Prepare"
               /\ prepared.subject = right.subject
        BY <1>1, CommitCertificateHasPrepareCertificate
      <3>2. left.subject = prepared.subject
        BY <1>1, <2>2, <3>1, CommitExcludesConflictingLaterPrepare
      <3> QED BY <3>1, <3>2
    <2>3. CASE right.view < left.view
      <3>1. PICK prepared \in prepareQCs:
               /\ prepared.context = left.context
               /\ prepared.view = left.view
               /\ prepared.phase = "Prepare"
               /\ prepared.subject = left.subject
        BY <1>1, CommitCertificateHasPrepareCertificate
      <3>2. right.subject = prepared.subject
        BY <1>1, <2>3, <3>1, CommitExcludesConflictingLaterPrepare
      <3> QED BY <3>1, <3>2
    <2> QED BY <1>1, <2>1, <2>2, <2>3, SMT
  <1> QED BY <1>1

=============================================================================
