---- MODULE SumeragiV2AgreementLemmas ----
EXTENDS SumeragiV2Inductive, SumeragiV2QuorumProofs, NaturalsInduction

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
    <2>9. /\ prepareVote.signer \in Honest
          /\ commitVote.signer = prepareVote.signer
          /\ commitVote.context = prepareVote.context
          /\ commitVote.phase = "Commit"
          /\ commitVote.view < prepareVote.view
          /\ commitVote.subject # prepareVote.subject
      BY <1>1, <2>6, <2>7, <2>8 DEF VoteBacksCertificate
    <2>10. PrepareCarriesHigherSafeQc(prepareVote)
      BY <1>1, <2>8 DEF PrepareLineageSound
    <2>11. PICK carried \in prepareQCs:
              /\ carried.context = prepareVote.context
              /\ carried.phase = "Prepare"
              /\ commitVote.view < carried.view
              /\ carried.view < prepareVote.view
              /\ carried.subject = prepareVote.subject
      BY <2>9, <2>10 DEF PrepareCarriesHigherSafeQc
    <2> QED BY <2>7, <2>8, <2>11 DEF VoteBacksCertificate
  <1> QED BY <1>1

=============================================================================
