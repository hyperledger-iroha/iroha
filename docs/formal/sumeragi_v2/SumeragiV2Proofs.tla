---- MODULE SumeragiV2Proofs ----
EXTENDS SumeragiV2InductiveProofs

(***************************************************************************
TLAPS proof ledger for the production-aligned Sumeragi v2 model.

Only statements carrying THEOREM below are claimed as mechanically checked.
The compositional kernel theorems live in SumeragiV2QuorumProofs and
SumeragiV2SafetyLemmas.  Every unfinished end-to-end result is an explicitly
named predicate, not a theorem.  proof_coverage.json is the release ledger.
***************************************************************************)

THEOREM QcValidityCarriesDualQuorum ==
  \A qc \in QcRecordSet:
    QcValid(qc) => DualQuorum(CurrentEpoch, qc.signers)
BY DEF QcValid

THEOREM TcValidityCarriesDisjointDualQuorum ==
  \A tc \in TcRecordSet:
    TCValid(tc)
      => /\ TimeoutVotesDisjoint(tc.votes)
         /\ DualQuorum(CurrentEpoch, TimeoutSignerSet(tc.votes))
BY DEF TCValid

THEOREM SafePrepareOnLockedSubject ==
  \A node \in ValidatorIds, proposal \in ProposalRecordSet:
    lockSubject[node] = proposal.subject => SafeToPrepare(node, proposal)
BY DEF SafeToPrepare

THEOREM HigherPrepareReleasesDifferentLock ==
  \A node \in ValidatorIds, proposal \in ProposalRecordSet:
    (proposal.justifyRank > lockRank[node]
      /\ proposal.justifySubject = proposal.subject)
      => SafeToPrepare(node, proposal)
BY DEF SafeToPrepare

THEOREM PersistedPrepareIsRequiredBeforePrepareSigning ==
  PrepareSigningRequiresIntent
    <=> \A request \in signVotes:
          request.vote.phase = "Prepare" => request.vote \in prepareIntents
BY DEF PrepareSigningRequiresIntent

THEOREM PersistedDecisionIsRequiredBeforeApplication ==
  AppliedRequiresDecision <=> applied \subseteq decisions
BY DEF AppliedRequiresDecision

THEOREM QuorumIntersectionObligation ==
  QuorumConfiguration
    => /\ CountQuorumIntersectionHasHonest
       /\ PowerQuorumIntersectionHasHonest
       /\ DualQuorumIntersectionHasHonest
BY AllQuorumIntersectionForms

InductiveInvariant ==
  /\ Safety
  /\ ContextIdentityBindsFrozenEpoch
  /\ OldContextCertificateRejected
  /\ ContextParentWasApplied

InitialStateObligation == Init => InductiveInvariant

THEOREM InitialStateEstablishesInductiveInvariant ==
  InitialStateObligation
PROOF
  <1>1. ASSUME Init
         PROVE InductiveInvariant
    <2>1. TypeInvariant
      BY <1>1, Isa
         DEF Init, TypeInvariant, Heights, Views, Generations, Ranks,
             ContextRecords, ContextRecord, ExpectedEpoch
    <2>2. OnePendingPersistencePerNode
      BY <1>1, Isa DEF Init, OnePendingPersistencePerNode
    <2>3. /\ ProposalSigningRequiresIntent
          /\ PrepareSigningRequiresIntent
          /\ CommitSigningRequiresIntent
          /\ TimeoutSigningRequiresIntent
      BY <1>1, Isa
         DEF Init, ProposalSigningRequiresIntent,
             PrepareSigningRequiresIntent, CommitSigningRequiresIntent,
             TimeoutSigningRequiresIntent
    <2>4. /\ HonestPrepareUniqueness
          /\ HonestCommitUniqueness
          /\ HonestTimeoutUniqueness
          /\ DecisionAgreement
          /\ AppliedRequiresDecision
      BY <1>1, Isa
         DEF Init, HonestPrepareUniqueness, HonestCommitUniqueness,
             HonestTimeoutUniqueness, DecisionAgreement,
             AppliedRequiresDecision
    <2>5. LockBelowHighest
      BY <1>1 DEF Init, LockBelowHighest, NoRank
    <2>6. Safety
      BY <2>1, <2>2, <2>3, <2>4, <2>5 DEF Safety
    <2>7. ContextIdentityBindsFrozenEpoch
      BY ContextRecordCarriesFrozenEpoch
         DEF ContextIdentityBindsFrozenEpoch, ContextRecords,
             ContextRecord
    <2>8. OldContextCertificateRejected
      BY <1>1, Isa DEF Init, OldContextCertificateRejected
    <2>9. ContextParentWasApplied
      BY <1>1, Isa DEF Init, ContextParentWasApplied, ContextRecord,
                            ExpectedEpoch
    <2> QED BY <2>6, <2>7, <2>8, <2>9 DEF InductiveInvariant
  <1> QED BY <1>1 DEF InitialStateObligation

ActionPreservationObligation ==
  InductiveInvariant /\ [NextV2]_vars => InductiveInvariant'

DurableVoteUniquenessObligation ==
  Spec => [](/\ HonestPrepareUniqueness
             /\ HonestCommitUniqueness
             /\ HonestTimeoutUniqueness)

LockMonotonicityAction ==
  \A node \in ValidatorIds:
    /\ (context' = context => lockRank'[node] >= lockRank[node])
    /\ (context' = context
         /\ lockSubject'[node] # lockSubject[node]
         => lockRank'[node] > lockRank[node])

LockMonotonicityObligation ==
  InductiveInvariant /\ [NextV2]_vars => LockMonotonicityAction

THEOREM PersistLockCommitIsLockMonotone ==
  \A request \in LockCommitWalSet:
    TypeInvariant /\ PersistLockCommit(request)
      => LockMonotonicityAction
BY SMT DEF PersistLockCommit, LockMonotonicityAction

THEOREM PersistInstallTCIsLockMonotone ==
  \A request:
    TypeInvariant /\ PersistInstallTC(request)
      => LockMonotonicityAction
BY SMT DEF PersistInstallTC, LockMonotonicityAction,
           TcHighRank, HighestTimeoutVote

PrepareCertificateAvailability ==
  \A qc \in prepareQCs:
    \E signer \in qc.signers \cap Honest:
      BodyHeldBy(durableBodies, signer, qc.context, qc.subject)

CommitCertificateAvailability ==
  \A qc \in commitQCs:
    \E signer \in qc.signers \cap Honest:
      BodyHeldBy(durableBodies, signer, qc.context, qc.subject)

AvailabilityObligation ==
  Spec => [](/\ PrepareCertificateAvailability
             /\ CommitCertificateAvailability)

ExternalValidityObligation ==
  Spec => [](/\ \A qc \in prepareQCs: qc.subject \in ValidSubjects
             /\ \A qc \in commitQCs: qc.subject \in ValidSubjects
             /\ \A decision \in decisions:
                  decision.qc.subject \in ValidSubjects)

PrepareCertificateUniqueness ==
  \A left, right \in prepareQCs:
    (left.context = right.context /\ left.view = right.view)
      => left.subject = right.subject

CommitCertificateUniqueness ==
  \A left, right \in commitQCs:
    (left.context = right.context /\ left.view = right.view)
      => left.subject = right.subject

CertificateUniquenessObligation ==
  Spec => [](/\ PrepareCertificateUniqueness
             /\ CommitCertificateUniqueness)

PotentialCommitSigners(roundView, subject) ==
  {vote.signer:
    vote \in {candidate \in commitIntents:
      /\ candidate.context = context
      /\ candidate.view = roundView
      /\ candidate.subject = subject}}

TCProtectsPotentialCommit(tc) ==
  \A protectedView \in 0..tc.view, subject \in Subjects:
    DualQuorum(CurrentEpoch,
      PotentialCommitSigners(protectedView, subject))
      => /\ TcHighRank(tc) >= protectedView
         /\ (TcHighRank(tc) = protectedView
               => TcHighSubject(tc) = subject)

TimeoutProtectionObligation ==
  Spec => [](\A tc \in formedTCs: TCProtectsPotentialCommit(tc))

AgreementObligation == Spec => []DecisionAgreement

NoConflictingCommitCertificatesObligation ==
  Spec => [](\A left, right \in commitQCs:
    left.context = right.context => left.subject = right.subject)

ChainPrefixObligation ==
  Spec => []ContextParentWasApplied

CrashRecoveryObligation ==
  Spec => [](/\ CrashPreservesDurableProjection
             /\ RestartPreservesDurableProjection
             /\ PendingWritesAreUnacknowledged
             /\ ProposalSigningRequiresIntent
             /\ PrepareSigningRequiresIntent
             /\ CommitSigningRequiresIntent
             /\ TimeoutSigningRequiresIntent
             /\ AppliedRequiresDecision)

EpochBoundaryObligation == Spec => []EpochBoundarySafety

NodeHasDecision(node) ==
  \E decision \in decisions:
    /\ decision.node = node
    /\ decision.qc.context = context

ResponsiveNodesDecide ==
  \A node \in Responsive \cap CurrentVoters: NodeHasDecision(node)

ResponsiveNodesApply ==
  \A node \in Responsive \cap CurrentVoters:
    \E application \in applied:
      /\ application.node = node
      /\ application.qc.context = context

TimeoutViewProgressObligation ==
  Spec => \A node \in Responsive \cap CurrentVoters, roundView \in Views:
    (gst /\ nodeView[node] = roundView /\ ~NodeHasDecision(node))
      ~> (nodeView[node] > roundView \/ NodeHasDecision(node))

RotatingLeaderProgressObligation ==
  Spec => (gst /\ ~ResponsiveNodesDecide) ~> ResponsiveNodesDecide

ApplicationLivenessObligation ==
  Spec => (gst /\ ResponsiveNodesDecide) ~> ResponsiveNodesApply

HeightLivenessObligation ==
  Spec => \A blockHeight \in Heights:
    (gst /\ height = blockHeight)
      ~> (height > blockHeight \/
           (blockHeight = MaxHeight /\ ResponsiveNodesApply))

=============================================================================
