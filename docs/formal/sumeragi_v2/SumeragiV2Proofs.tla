---- MODULE SumeragiV2Proofs ----
EXTENDS SumeragiV2InductiveProofs

(***************************************************************************
Temporal closure of the production-aligned Sumeragi v2 safety proof.

The action-by-action induction is discharged in SumeragiV2InductiveProofs.
This module closes that induction over Spec and exposes the release safety
properties as temporal corollaries.  Liveness and the asynchronous history
refinement live in separate modules so that safety never depends on fairness.
proof_coverage.json remains the authoritative record of backend evidence.
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
    <2>1. Safety
      BY <1>1, InitEstablishesReleaseSafety
    <2>2. /\ ContextIdentityBindsFrozenEpoch
          /\ OldContextCertificateRejected
          /\ ContextParentWasApplied
      BY <1>1, InitEstablishesContextSafety
    <2> QED BY <2>1, <2>2 DEF InductiveInvariant
  <1> QED BY <1>1 DEF InitialStateObligation

ActionPreservationObligation ==
  InductiveInvariant /\ [NextV2]_vars => InductiveInvariant'

THEOREM SpecImpliesAlwaysStrongInductiveInvariant ==
  Spec => []StrongInductiveInvariant
PROOF
  <1>1. Init => StrongInductiveInvariant
    BY InitEstablishesStrongInductiveInvariant
  <1>2. StrongInductiveInvariant /\ [NextV2]_vars
           => StrongInductiveInvariant'
    BY StrongInductiveActionPreservation
  <1> QED BY <1>1, <1>2, PTL DEF Spec

THEOREM SpecImpliesAlwaysInductiveInvariant ==
  Spec => []InductiveInvariant
PROOF
  <1>1. StrongInductiveInvariant => InductiveInvariant
    BY DEF StrongInductiveInvariant, InductiveInvariant
  <1> QED BY <1>1, SpecImpliesAlwaysStrongInductiveInvariant, PTL

THEOREM DurableVoteUniquenessObligation ==
  Spec => [](/\ HonestPrepareUniqueness
             /\ HonestCommitUniqueness
             /\ HonestTimeoutUniqueness)
PROOF
  <1>1. StrongInductiveInvariant
           => /\ HonestPrepareUniqueness
              /\ HonestCommitUniqueness
              /\ HonestTimeoutUniqueness
    BY DEF StrongInductiveInvariant, Safety
  <1> QED BY <1>1, SpecImpliesAlwaysStrongInductiveInvariant, PTL

LockMonotonicityAction ==
  \A node \in ValidatorIds:
    /\ (context' = context => lockRank'[node] >= lockRank[node])
    /\ (context' = context
         /\ lockSubject'[node] # lockSubject[node]
         => lockRank'[node] > lockRank[node])

THEOREM PersistLockCommitIsLockMonotone ==
  \A request:
    TypeInvariant /\ PendingVoteWritesAuthorized
      /\ PersistLockCommit(request)
      => LockMonotonicityAction
BY SMT
   DEF TypeInvariant, PendingVoteWritesAuthorized, PersistLockCommit,
       LockMonotonicityAction, LockCommitWalSet, QcRecordSet, Views, Ranks

THEOREM PersistInstallTCIsLockMonotone ==
  \A request:
    TypeInvariant /\ PersistInstallTC(request)
      => LockMonotonicityAction
BY SMT
   DEF TypeInvariant, PersistInstallTC, LockMonotonicityAction, Ranks

THEOREM LockMonotonicityObligation ==
  StrongInductiveInvariant /\ [NextV2]_vars => LockMonotonicityAction
BY IsaM("blast"), PersistLockCommitIsLockMonotone,
   PersistInstallTCIsLockMonotone
   DEF StrongInductiveInvariant, NextV2, Next, vars,
       LockMonotonicityAction, AdvanceContext,
       SetGST, AssembleLocalBody, BeginLocalProposal, PersistProposal,
       CompleteProposalSignature, DeliverProposal, FetchBody, StoreBody,
       ValidateBody, RejectBody, BeginPrepare, PersistPrepare,
       CompleteVoteSignature, ByzantineBroadcastVote, DeliverVote,
       FormPrepareQC, DeliverQC, BeginObservePrepare,
       PersistObservePrepare, BeginLockCommit, FormCommitQC,
       BeginDecision, PersistDecision, BeginTimeout, PersistTimeout,
       CompleteTimeoutSignature, ByzantineBroadcastTimeout,
       DeliverTimeout, FormTC, DeliverTC, BeginInstallTC,
       FetchCertifiedBody, ApplyDecision, Crash, Restart, ResumeProposal,
       ResumeVote, ResumeTimeout, DropProposal

PrepareCertificateAvailability ==
  \A qc \in prepareQCs:
    \E signer \in qc.signers \cap Honest:
      BodyHeldBy(durableBodies, signer, qc.context, qc.subject)

CommitCertificateAvailability ==
  \A qc \in commitQCs:
    \E signer \in qc.signers \cap Honest:
      BodyHeldBy(durableBodies, signer, qc.context, qc.subject)

CertificateValidityAndAvailabilityInvariant ==
  /\ \A qc \in prepareQCs:
       CertificateValidityAndAvailability(qc, durableBodies, ValidSubjects)
  /\ \A qc \in commitQCs:
       CertificateValidityAndAvailability(qc, durableBodies, ValidSubjects)

THEOREM StrongInvariantImpliesCertificateValidityAndAvailability ==
  StrongInductiveInvariant => CertificateValidityAndAvailabilityInvariant
PROOF
  <1>1. ASSUME StrongInductiveInvariant
         PROVE CertificateValidityAndAvailabilityInvariant
    <2>1. /\ QuorumConfiguration
          /\ CertificatesBackedByIntents
          /\ HonestDurableIntentsSound
      BY <1>1
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             Safety, TypeInvariant, ModelConfiguration
    <2>2. \A qc \in prepareQCs:
             CertificateValidityAndAvailability(
               qc, durableBodies, ValidSubjects)
      <3>1. ASSUME NEW qc \in prepareQCs
             PROVE CertificateValidityAndAvailability(
                     qc, durableBodies, ValidSubjects)
        <4>1. /\ qc.context.epoch \in Epochs
              /\ CertificateBackedBy(qc.context.epoch, qc,
                                     prepareIntents)
          BY <1>1, <3>1
             DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
                 CertificatesBackedByIntents, HistoricalQcValid
        <4>2. HonestIntentSound(
                 prepareIntents, durableBodies, ValidSubjects)
          BY <2>1 DEF HonestDurableIntentsSound
        <4> QED BY <2>1, <4>1, <4>2,
                      BackedCertificateIsValidAndAvailable
      <3> QED BY <3>1
    <2>3. \A qc \in commitQCs:
             CertificateValidityAndAvailability(
               qc, durableBodies, ValidSubjects)
      <3>1. ASSUME NEW qc \in commitQCs
             PROVE CertificateValidityAndAvailability(
                     qc, durableBodies, ValidSubjects)
        <4>1. /\ qc.context.epoch \in Epochs
              /\ CertificateBackedBy(qc.context.epoch, qc,
                                     commitIntents)
          BY <1>1, <3>1
             DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
                 CertificatesBackedByIntents, HistoricalQcValid
        <4>2. HonestIntentSound(
                 commitIntents, durableBodies, ValidSubjects)
          BY <2>1 DEF HonestDurableIntentsSound
        <4> QED BY <2>1, <4>1, <4>2,
                      BackedCertificateIsValidAndAvailable
      <3> QED BY <3>1
    <2> QED BY <2>2, <2>3
       DEF CertificateValidityAndAvailabilityInvariant
  <1> QED BY <1>1

THEOREM AvailabilityObligation ==
  Spec => [](/\ PrepareCertificateAvailability
             /\ CommitCertificateAvailability)
PROOF
  <1>1. CertificateValidityAndAvailabilityInvariant
           => /\ PrepareCertificateAvailability
              /\ CommitCertificateAvailability
    BY DEF CertificateValidityAndAvailabilityInvariant,
           CertificateValidityAndAvailability,
           PrepareCertificateAvailability, CommitCertificateAvailability
  <1> QED BY <1>1,
              StrongInvariantImpliesCertificateValidityAndAvailability,
              SpecImpliesAlwaysStrongInductiveInvariant, PTL

THEOREM ExternalValidityObligation ==
  Spec => [](/\ \A qc \in prepareQCs: qc.subject \in ValidSubjects
             /\ \A qc \in commitQCs: qc.subject \in ValidSubjects
             /\ \A decision \in decisions:
                  decision.qc.subject \in ValidSubjects)
PROOF
  <1>1. StrongInductiveInvariant
           => /\ \A qc \in prepareQCs:
                    qc.subject \in ValidSubjects
              /\ \A qc \in commitQCs:
                    qc.subject \in ValidSubjects
              /\ \A decision \in decisions:
                    decision.qc.subject \in ValidSubjects
    BY StrongInvariantImpliesCertificateValidityAndAvailability
       DEF StrongInductiveInvariant, Safety, DecisionAgreement,
           CertificateValidityAndAvailabilityInvariant,
           CertificateValidityAndAvailability
  <1> QED BY <1>1, SpecImpliesAlwaysStrongInductiveInvariant, PTL

PrepareCertificateUniqueness ==
  \A left, right \in prepareQCs:
    (left.context = right.context /\ left.view = right.view)
      => left.subject = right.subject

CommitCertificateUniqueness ==
  \A left, right \in commitQCs:
    (left.context = right.context /\ left.view = right.view)
      => left.subject = right.subject

CertificateUniquenessInvariant ==
  /\ PrepareCertificateUniqueness
  /\ CommitCertificateUniqueness

THEOREM StrongInvariantImpliesCertificateUniqueness ==
  StrongInductiveInvariant => CertificateUniquenessInvariant
PROOF
  <1>1. ASSUME StrongInductiveInvariant
         PROVE CertificateUniquenessInvariant
    <2>1. /\ QuorumConfiguration
          /\ CertificatesBackedByIntents
          /\ HonestVoteUnique(prepareIntents)
          /\ HonestVoteUnique(commitIntents)
          /\ CertificatePhasesCorrect
      BY <1>1
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             ModelConfiguration, ReducerProvenanceInvariant,
             LineageInvariant
    <2>2. PrepareCertificateUniqueness
      <3>1. ASSUME NEW left \in prepareQCs,
                    NEW right \in prepareQCs,
                    left.context = right.context,
                    left.view = right.view
             PROVE left.subject = right.subject
        <4>1. /\ left.context.epoch \in Epochs
              /\ CertificateBackedBy(left.context.epoch, left,
                                     prepareIntents)
              /\ CertificateBackedBy(left.context.epoch, right,
                                     prepareIntents)
              /\ SameCertificateSlot(left, right)
          BY <1>1, <2>1, <3>1
             DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
                 CertificatesBackedByIntents, HistoricalQcValid,
                 CertificatePhasesCorrect, SameCertificateSlot
        <4> QED BY <2>1, <4>1, SameViewCertificateUniqueness
      <3> QED BY <3>1 DEF PrepareCertificateUniqueness
    <2>3. CommitCertificateUniqueness
      <3>1. ASSUME NEW left \in commitQCs,
                    NEW right \in commitQCs,
                    left.context = right.context,
                    left.view = right.view
             PROVE left.subject = right.subject
        <4>1. /\ left.context.epoch \in Epochs
              /\ CertificateBackedBy(left.context.epoch, left,
                                     commitIntents)
              /\ CertificateBackedBy(left.context.epoch, right,
                                     commitIntents)
              /\ SameCertificateSlot(left, right)
          BY <1>1, <2>1, <3>1
             DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
                 CertificatesBackedByIntents, HistoricalQcValid,
                 CertificatePhasesCorrect, SameCertificateSlot
        <4> QED BY <2>1, <4>1, SameViewCertificateUniqueness
      <3> QED BY <3>1 DEF CommitCertificateUniqueness
    <2> QED BY <2>2, <2>3 DEF CertificateUniquenessInvariant
  <1> QED BY <1>1

THEOREM CertificateUniquenessObligation ==
  Spec => []CertificateUniquenessInvariant
BY StrongInvariantImpliesCertificateUniqueness,
   SpecImpliesAlwaysStrongInductiveInvariant, PTL

PotentialCommitSigners(certificateContext, roundView, subject) ==
  CommitSignerSet(
    commitIntents, certificateContext, roundView, subject)

TCProtectsPotentialCommit(tc) ==
  \A protectedView \in 0..tc.view, subject \in Subjects:
    DualQuorum(tc.context.epoch,
      PotentialCommitSigners(tc.context, protectedView, subject))
      => /\ TcHighRank(tc) >= protectedView
         /\ (TcHighRank(tc) = protectedView
               => TcHighSubject(tc) = subject)

THEOREM StrongInvariantBuildsTimeoutProtectionKernel ==
  StrongInductiveInvariant
    => \A tc \in formedTCs,
          protectedView \in 0..tc.view,
          subject \in Subjects:
         DualQuorum(tc.context.epoch,
           PotentialCommitSigners(tc.context, protectedView, subject))
           => TimeoutProtectionKernel(
                tc.context.epoch, tc, commitIntents,
                protectedView, subject)
BY IsaMT("blast", 120)
   DEF StrongInductiveInvariant, Safety, TypeInvariant,
       ReducerProvenanceInvariant, FormedTimeoutCertificatesSound,
       DurableTimeoutsProtectCommits, TimeoutProtectionKernel,
       TimeoutRanksTyped, PotentialCommitSigners, CommitSignerSet,
       Ranks, Views, TcHighRank, HighestTimeoutVote

THEOREM StrongInvariantImpliesTimeoutProtection ==
  StrongInductiveInvariant
    => \A tc \in formedTCs: TCProtectsPotentialCommit(tc)
PROOF
  <1>1. ASSUME StrongInductiveInvariant,
              NEW tc \in formedTCs,
              NEW protectedView \in 0..tc.view,
              NEW subject \in Subjects,
              DualQuorum(tc.context.epoch,
                PotentialCommitSigners(
                  tc.context, protectedView, subject))
         PROVE /\ TcHighRank(tc) >= protectedView
               /\ (TcHighRank(tc) = protectedView
                     => TcHighSubject(tc) = subject)
    <2>1. QuorumConfiguration
      BY <1>1
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             ModelConfiguration
    <2>2. tc.context.epoch \in Epochs
      BY <1>1
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             FormedTimeoutCertificatesSound
    <2>3. TimeoutProtectionKernel(
             tc.context.epoch, tc, commitIntents,
             protectedView, subject)
      BY <1>1, StrongInvariantBuildsTimeoutProtectionKernel
    <2> QED BY <2>1, <2>2, <2>3,
                  GroupedTimeoutProtectsCommitQuorum
       DEF TCProtectsViewSubject
  <1> QED BY <1>1 DEF TCProtectsPotentialCommit

THEOREM TimeoutProtectionObligation ==
  Spec => [](\A tc \in formedTCs: TCProtectsPotentialCommit(tc))
BY StrongInvariantImpliesTimeoutProtection,
   SpecImpliesAlwaysStrongInductiveInvariant, PTL

THEOREM StrongInvariantImpliesCommitCertificateAgreement ==
  StrongInductiveInvariant
    => \A left, right \in commitQCs:
         left.context = right.context => left.subject = right.subject
BY CommitCertificateAgreement
   DEF StrongInductiveInvariant, Safety, TypeInvariant, ModelConfiguration,
       ReducerProvenanceInvariant, LineageInvariant

THEOREM AgreementObligation ==
  Spec => []DecisionAgreement
PROOF
  <1>1. StrongInductiveInvariant => DecisionAgreement
    BY DEF StrongInductiveInvariant, Safety
  <1> QED BY <1>1, SpecImpliesAlwaysStrongInductiveInvariant, PTL

THEOREM NoConflictingCommitCertificatesObligation ==
  Spec => [](\A left, right \in commitQCs:
    left.context = right.context => left.subject = right.subject)
BY StrongInvariantImpliesCommitCertificateAgreement,
   SpecImpliesAlwaysStrongInductiveInvariant, PTL

THEOREM ChainPrefixObligation ==
  Spec => []ContextParentWasApplied
PROOF
  <1>1. StrongInductiveInvariant => ContextParentWasApplied
    BY DEF StrongInductiveInvariant
  <1> QED BY <1>1, SpecImpliesAlwaysStrongInductiveInvariant, PTL

CrashRecoveryStateInvariant ==
  /\ ProposalSigningRequiresIntent
  /\ PrepareSigningRequiresIntent
  /\ CommitSigningRequiresIntent
  /\ TimeoutSigningRequiresIntent
  /\ AppliedRequiresDecision

THEOREM CrashAndRestartPreserveDurableSafety ==
  /\ CrashPreservesDurableProjection
  /\ RestartPreservesDurableProjection
  /\ PendingWritesAreUnacknowledged
  /\ StaleGenerationRejected
BY DEF CrashPreservesDurableProjection,
       RestartPreservesDurableProjection,
       PendingWritesAreUnacknowledged, StaleGenerationRejected,
       DurableProjection, DurableProjectionPrime, Crash, Restart

THEOREM CrashRecoveryObligation ==
  /\ Spec => []CrashRecoveryStateInvariant
  /\ CrashPreservesDurableProjection
  /\ RestartPreservesDurableProjection
  /\ PendingWritesAreUnacknowledged
  /\ StaleGenerationRejected
PROOF
  <1>1. StrongInductiveInvariant => CrashRecoveryStateInvariant
    BY DEF StrongInductiveInvariant, Safety,
           CrashRecoveryStateInvariant
  <1>2. Spec => []CrashRecoveryStateInvariant
    BY <1>1, SpecImpliesAlwaysStrongInductiveInvariant, PTL
  <1> QED BY <1>2, CrashAndRestartPreserveDurableSafety

THEOREM EpochBoundaryObligation ==
  Spec => []EpochBoundarySafety
PROOF
  <1>1. StrongInductiveInvariant => EpochBoundarySafety
    BY DEF StrongInductiveInvariant, EpochBoundarySafety
  <1> QED BY <1>1, SpecImpliesAlwaysStrongInductiveInvariant, PTL

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

DecisionBodyReady(node, qc) ==
  /\ BodyHeldBy(durableBodies, node, qc.context, qc.subject)
  /\ \E validation \in validatedBodies:
       /\ validation.node = node
       /\ validation.context = qc.context
       /\ validation.subject = qc.subject

TimeoutViewProgressProperty(specification) ==
  specification
    => \A node \in Responsive \cap CurrentVoters,
          roundView \in Views:
         (gst /\ nodeView[node] = roundView /\ ~NodeHasDecision(node))
           ~> (nodeView[node] > roundView \/ NodeHasDecision(node))

RotatingLeaderProgressProperty(specification) ==
  specification
    => (gst /\ ~ResponsiveNodesDecide) ~> ResponsiveNodesDecide

ApplicationLivenessProperty(specification) ==
  specification
    => (gst /\ ResponsiveNodesDecide) ~> ResponsiveNodesApply

HeightLivenessProperty(specification) ==
  specification
    => \A blockHeight \in Heights:
         (gst /\ height = blockHeight)
           ~> (height > blockHeight \/
                (blockHeight = MaxHeight /\ ResponsiveNodesApply))

=============================================================================
