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

THEOREM CoreSpecAtAlwaysStrongInductiveInvariant ==
  \A initialContext:
    CoreSpecAt(initialContext) => []StrongInductiveInvariant
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE CoreSpecAt(initialContext)
                 => []StrongInductiveInvariant
    <2>1. InitAt(initialContext) => StrongInductiveInvariant
      BY InitAtEstablishesStrongInductiveInvariant
    <2>2. StrongInductiveInvariant /\ [Next]_vars
             => StrongInductiveInvariant'
      BY CoreStrongInductiveActionPreservation
    <2> QED BY <2>1, <2>2, PTL DEF CoreSpecAt
  <1> QED BY <1>1

THEOREM SpecImpliesAlwaysInductiveInvariant ==
  Spec => []InductiveInvariant
PROOF
  <1>1. StrongInductiveInvariant => InductiveInvariant
    BY DEF StrongInductiveInvariant, InductiveInvariant
  <1> QED BY <1>1, SpecImpliesAlwaysStrongInductiveInvariant, PTL

DurableVoteUniquenessProperty(specification) ==
  specification => [](/\ HonestPrepareUniqueness
                       /\ HonestCommitUniqueness
                       /\ HonestTimeoutUniqueness)

THEOREM DurableVoteUniquenessObligation ==
  \A initialContext:
    DurableVoteUniquenessProperty(CoreSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE DurableVoteUniquenessProperty(CoreSpecAt(initialContext))
    <2>1. CoreSpecAt(initialContext) => []StrongInductiveInvariant
      BY CoreSpecAtAlwaysStrongInductiveInvariant
    <2>2. StrongInductiveInvariant
             => /\ HonestPrepareUniqueness
                /\ HonestCommitUniqueness
                /\ HonestTimeoutUniqueness
      BY DEF StrongInductiveInvariant, Safety
    <2> QED BY <2>1, <2>2, PTL DEF DurableVoteUniquenessProperty
  <1> QED BY <1>1

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

UnchangedContextAndLocks ==
  UNCHANGED <<context, lockRank, lockSubject>>

THEOREM UnchangedContextAndLocksIsLockMonotone ==
  TypeInvariant /\ UnchangedContextAndLocks => LockMonotonicityAction
BY SMT
   DEF TypeInvariant, UnchangedContextAndLocks,
       LockMonotonicityAction, Ranks

(***************************************************************************
Every reducer action other than the two durable lock writes leaves the
current context and lock projection unchanged.  Keeping this footprint as an
explicit disjunction makes the temporal lock proof auditable and avoids
asking one backend invocation to expand the entire reducer relation.
***************************************************************************)
LockStableNext ==
  \/ SetGST
  \/ \E node \in ValidatorIds, subject \in Subjects:
       AssembleLocalBody(node, subject)
  \/ \E node \in ValidatorIds, subject \in Subjects:
       BeginLocalProposal(node, subject)
  \/ \E request \in pendingProposal: PersistProposal(request)
  \/ \E request \in signProposals: CompleteProposalSignature(request)
  \/ \E signer \in ValidatorIds, roundView \in Views,
       subject \in Subjects, justifyRank \in Ranks,
       justifySubject \in SubjectOrNone:
       ByzantineBroadcastProposal(signer, roundView, subject,
                                  justifyRank, justifySubject)
  \/ \E envelope \in proposalNetwork: DeliverProposal(envelope)
  \/ \E node \in ValidatorIds, proposal \in SeenProposalValues:
       FetchBody(node, proposal)
  \/ \E node \in ValidatorIds, subject \in Subjects: StoreBody(node, subject)
  \/ \E node \in ValidatorIds, proposal \in SeenProposalValues:
       ValidateBody(node, proposal) \/ RejectBody(node, proposal)
  \/ \E node \in ValidatorIds, proposal \in SeenProposalValues:
       BeginPrepare(node, proposal)
  \/ \E request \in pendingPrepare: PersistPrepare(request)
  \/ \E request \in signVotes: CompleteVoteSignature(request)
  \/ \E signer \in ValidatorIds, roundView \in Views,
       phase \in Phases, subject \in Subjects:
       ByzantineBroadcastVote(signer, roundView, phase, subject)
  \/ \E envelope \in voteNetwork: DeliverVote(envelope)
  \/ \E node \in ValidatorIds, roundView \in Views, subject \in Subjects:
       FormPrepareQC(node, roundView, subject)
  \/ \E envelope \in qcNetwork: DeliverQC(envelope)
  \/ \E node \in ValidatorIds, qc \in ReceivedQcValues:
       BeginObservePrepare(node, qc)
  \/ \E request \in pendingObservePrepare:
       PersistObservePrepare(request)
  \/ \E node \in ValidatorIds, qc \in ReceivedQcValues:
       BeginLockCommit(node, qc)
  \/ \E node \in ValidatorIds, roundView \in Views, subject \in Subjects:
       FormCommitQC(node, roundView, subject)
  \/ \E node \in ValidatorIds, qc \in ReceivedQcValues:
       BeginDecision(node, qc)
  \/ \E request \in pendingDecision: PersistDecision(request)
  \/ \E node \in ValidatorIds: BeginTimeout(node)
  \/ \E request \in pendingTimeout: PersistTimeout(request)
  \/ \E request \in signTimeouts: CompleteTimeoutSignature(request)
  \/ \E signer \in ValidatorIds, roundView \in Views,
       highRank \in Ranks, highSubject \in SubjectOrNone:
       ByzantineBroadcastTimeout(signer, roundView, highRank, highSubject)
  \/ \E envelope \in timeoutNetwork: DeliverTimeout(envelope)
  \/ \E node \in ValidatorIds, roundView \in Views: FormTC(node, roundView)
  \/ \E envelope \in tcNetwork: DeliverTC(envelope)
  \/ \E node \in ValidatorIds, tc \in ReceivedTcValues:
       BeginInstallTC(node, tc)
  \/ \E node \in ValidatorIds, qc \in DecisionQcValues:
       FetchCertifiedBody(node, qc)
  \/ \E node \in ValidatorIds, qc \in DecisionQcValues:
       ApplyDecision(node, qc)
  \/ \E node \in ValidatorIds: Crash(node) \/ Restart(node)
  \/ \E node \in ValidatorIds, proposal \in proposalIntents:
       ResumeProposal(node, proposal)
  \/ \E node \in ValidatorIds,
       vote \in prepareIntents \cup commitIntents:
       ResumeVote(node, vote)
  \/ \E node \in ValidatorIds, vote \in timeoutIntents:
       ResumeTimeout(node, vote)
  \/ \E envelope \in proposalNetwork: DropProposal(envelope)

THEOREM NextLockFootprintClassification ==
  Next
    => \/ LockStableNext
       \/ (\E request \in pendingLockCommit: PersistLockCommit(request))
       \/ (\E request \in pendingInstallTC: PersistInstallTC(request))
BY DEF Next, LockStableNext

THEOREM LockStableNextLeavesContextAndLocks ==
  LockStableNext => UnchangedContextAndLocks
BY IsaM("blast")
   DEF LockStableNext, UnchangedContextAndLocks,
       SetGST, AssembleLocalBody, BeginLocalProposal, PersistProposal,
       CompleteProposalSignature, ByzantineBroadcastProposal,
       DeliverProposal, FetchBody, StoreBody,
       ValidateBody, RejectBody, BeginPrepare, PersistPrepare,
       CompleteVoteSignature, ByzantineBroadcastVote, DeliverVote,
       FormPrepareQC, DeliverQC, BeginObservePrepare,
       PersistObservePrepare, BeginLockCommit, FormCommitQC,
       BeginDecision, PersistDecision, BeginTimeout, PersistTimeout,
       CompleteTimeoutSignature, ByzantineBroadcastTimeout,
       DeliverTimeout, FormTC, DeliverTC, BeginInstallTC,
       FetchCertifiedBody, ApplyDecision, Crash, Restart, ResumeProposal,
       ResumeVote, ResumeTimeout, DropProposal

THEOREM AdvanceContextEndsCurrentLockOrder ==
  \A subject \in Subjects:
    StrongInductiveInvariant /\ AdvanceContext(subject)
      => LockMonotonicityAction
PROOF
  <1>1. ASSUME NEW subject \in Subjects,
              StrongInductiveInvariant,
              AdvanceContext(subject)
         PROVE LockMonotonicityAction
    <2>1. /\ context.height = height
          /\ context'.height = height + 1
          /\ height \in Int
      BY <1>1, SMT
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             AdvanceContext, ContextRecord, Heights
    <2>2. context' # context
      BY <2>1, SMT
    <2> QED BY <2>2 DEF LockMonotonicityAction
  <1> QED BY <1>1

THEOREM StrongInvariantImpliesLockMonotonicityAction ==
  StrongInductiveInvariant /\ [Next]_vars => LockMonotonicityAction
PROOF
  <1>1. ASSUME StrongInductiveInvariant, [Next]_vars
         PROVE LockMonotonicityAction
    <2>1. CASE UNCHANGED vars
      BY <1>1, <2>1, UnchangedContextAndLocksIsLockMonotone
         DEF StrongInductiveInvariant, Safety, vars,
             UnchangedContextAndLocks
    <2>2. CASE Next
      <3>1. \/ LockStableNext
             \/ (\E request \in pendingLockCommit:
                   PersistLockCommit(request))
             \/ (\E request \in pendingInstallTC:
                   PersistInstallTC(request))
        BY <2>2, NextLockFootprintClassification
      <3>2. CASE LockStableNext
        BY <1>1, <3>2, LockStableNextLeavesContextAndLocks,
           UnchangedContextAndLocksIsLockMonotone
           DEF StrongInductiveInvariant, Safety
      <3>3. CASE \E request \in pendingLockCommit:
                     PersistLockCommit(request)
        <4>1. PICK request \in pendingLockCommit:
                 PersistLockCommit(request)
          BY <3>3
        <4> QED BY <1>1, <4>1, PersistLockCommitIsLockMonotone
           DEF StrongInductiveInvariant, Safety,
               ReducerProvenanceInvariant
      <3>4. CASE \E request \in pendingInstallTC:
                     PersistInstallTC(request)
        <4>1. PICK request \in pendingInstallTC:
                 PersistInstallTC(request)
          BY <3>4
        <4> QED BY <1>1, <4>1, PersistInstallTCIsLockMonotone
           DEF StrongInductiveInvariant, Safety
      <3> QED BY <3>1, <3>2, <3>3, <3>4
    <2> QED BY <1>1, <2>1, <2>2
  <1> QED BY <1>1

LockMonotonicityProperty(specification) ==
  specification => [][LockMonotonicityAction]_vars

THEOREM LockMonotonicityObligation ==
  \A initialContext:
    LockMonotonicityProperty(CoreSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE LockMonotonicityProperty(CoreSpecAt(initialContext))
    <2>1. CoreSpecAt(initialContext) => []StrongInductiveInvariant
      BY CoreSpecAtAlwaysStrongInductiveInvariant
    <2>2. StrongInductiveInvariant /\ [Next]_vars
             => LockMonotonicityAction
      BY StrongInvariantImpliesLockMonotonicityAction
    <2> QED BY <2>1, <2>2, PTL
       DEF LockMonotonicityProperty, CoreSpecAt
  <1> QED BY <1>1

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

CertifiedBodyAvailabilityProperty(specification) ==
  specification => [](/\ PrepareCertificateAvailability
                       /\ CommitCertificateAvailability)

THEOREM AvailabilityObligation ==
  \A initialContext:
    CertifiedBodyAvailabilityProperty(CoreSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE CertifiedBodyAvailabilityProperty(CoreSpecAt(initialContext))
    <2>1. CoreSpecAt(initialContext) => []StrongInductiveInvariant
      BY CoreSpecAtAlwaysStrongInductiveInvariant
    <2>2. StrongInductiveInvariant
             => /\ PrepareCertificateAvailability
                /\ CommitCertificateAvailability
      BY StrongInvariantImpliesCertificateValidityAndAvailability
         DEF CertificateValidityAndAvailabilityInvariant,
             CertificateValidityAndAvailability,
             PrepareCertificateAvailability, CommitCertificateAvailability
    <2> QED BY <2>1, <2>2, PTL
       DEF CertifiedBodyAvailabilityProperty
  <1> QED BY <1>1

ExternalValidityProperty(specification) ==
  specification
    => [](/\ \A qc \in prepareQCs: qc.subject \in ValidSubjects
          /\ \A qc \in commitQCs: qc.subject \in ValidSubjects
          /\ \A decision \in decisions:
               decision.qc.subject \in ValidSubjects)

THEOREM ExternalValidityObligation ==
  \A initialContext:
    ExternalValidityProperty(CoreSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE ExternalValidityProperty(CoreSpecAt(initialContext))
    <2>1. CoreSpecAt(initialContext) => []StrongInductiveInvariant
      BY CoreSpecAtAlwaysStrongInductiveInvariant
    <2>2. StrongInductiveInvariant
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
    <2> QED BY <2>1, <2>2, PTL DEF ExternalValidityProperty
  <1> QED BY <1>1

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

CertificateUniquenessProperty(specification) ==
  specification => []CertificateUniquenessInvariant

THEOREM CertificateUniquenessObligation ==
  \A initialContext:
    CertificateUniquenessProperty(CoreSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE CertificateUniquenessProperty(CoreSpecAt(initialContext))
    <2>1. CoreSpecAt(initialContext) => []StrongInductiveInvariant
      BY CoreSpecAtAlwaysStrongInductiveInvariant
    <2>2. StrongInductiveInvariant => CertificateUniquenessInvariant
      BY StrongInvariantImpliesCertificateUniqueness
    <2> QED BY <2>1, <2>2, PTL DEF CertificateUniquenessProperty
  <1> QED BY <1>1

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
    => \A tc \in formedTCs:
         \A protectedView \in 0..tc.view, subject \in Subjects:
           DualQuorum(tc.context.epoch,
             PotentialCommitSigners(tc.context, protectedView, subject))
             => TimeoutProtectionKernel(
                  tc.context.epoch, tc, commitIntents,
                  protectedView, subject)
PROOF
  <1>1. ASSUME StrongInductiveInvariant,
              NEW tc \in formedTCs,
              NEW protectedView \in 0..tc.view,
              NEW subject \in Subjects,
              DualQuorum(tc.context.epoch,
                PotentialCommitSigners(
                  tc.context, protectedView, subject))
         PROVE TimeoutProtectionKernel(
                 tc.context.epoch, tc, commitIntents,
                 protectedView, subject)
    <2>1. FormedTimeoutCertificatesSound
      BY <1>1 DEF StrongInductiveInvariant, ReducerProvenanceInvariant
    <2>2. TimeoutIntentProtectsCommits(tc.votes, commitIntents)
      BY <1>1, <2>1, IsaMT("blast", 120)
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             DurableTimeoutsProtectCommits,
             FormedTimeoutCertificatesSound,
             TimeoutIntentProtectsCommits,
             TimeoutVoteProtectsCommitSet
    <2>3. TimeoutRanksTyped(tc, protectedView)
      <3>1. HighestTimeoutVote(tc.votes) \in tc.votes
        BY <1>1, StrongInvariantImpliesTimeoutCertificateSelectorsSound
           DEF TimeoutCertificateSelectorsSound
      <3>2. /\ protectedView \in Int
             /\ HighestTimeoutVote(tc.votes).highRank \in Int
             /\ \A vote \in tc.votes: vote.highRank \in Int
        BY <1>1, <2>1, <3>1, SMT
           DEF FormedTimeoutCertificatesSound, Ranks, Views
      <3> QED BY <3>2 DEF TimeoutRanksTyped, TcHighRank
    <2> QED BY <1>1, <2>1, <2>2, <2>3, Isa
       DEF TimeoutProtectionKernel, PotentialCommitSigners,
           FormedTimeoutCertificatesSound
  <1> QED BY <1>1

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

TimeoutProtectionProperty(specification) ==
  specification
    => [](\A tc \in formedTCs: TCProtectsPotentialCommit(tc))

THEOREM TimeoutProtectionObligation ==
  \A initialContext:
    TimeoutProtectionProperty(CoreSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE TimeoutProtectionProperty(CoreSpecAt(initialContext))
    <2>1. CoreSpecAt(initialContext) => []StrongInductiveInvariant
      BY CoreSpecAtAlwaysStrongInductiveInvariant
    <2>2. StrongInductiveInvariant
             => \A tc \in formedTCs: TCProtectsPotentialCommit(tc)
      BY StrongInvariantImpliesTimeoutProtection
    <2> QED BY <2>1, <2>2, PTL DEF TimeoutProtectionProperty
  <1> QED BY <1>1

THEOREM StrongInvariantImpliesCommitCertificateAgreement ==
  StrongInductiveInvariant
    => \A left, right \in commitQCs:
         left.context = right.context => left.subject = right.subject
BY CommitCertificateAgreement
   DEF StrongInductiveInvariant, Safety, TypeInvariant, ModelConfiguration,
       ReducerProvenanceInvariant, LineageInvariant

AgreementProperty(specification) ==
  specification => []DecisionAgreement

THEOREM AgreementObligation ==
  \A initialContext:
    AgreementProperty(CoreSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE AgreementProperty(CoreSpecAt(initialContext))
    <2>1. CoreSpecAt(initialContext) => []StrongInductiveInvariant
      BY CoreSpecAtAlwaysStrongInductiveInvariant
    <2>2. StrongInductiveInvariant => DecisionAgreement
      BY DEF StrongInductiveInvariant, Safety
    <2> QED BY <2>1, <2>2, PTL DEF AgreementProperty
  <1> QED BY <1>1

NoConflictingCommitCertificatesProperty(specification) ==
  specification
    => [](\A left, right \in commitQCs:
          left.context = right.context => left.subject = right.subject)

THEOREM NoConflictingCommitCertificatesObligation ==
  \A initialContext:
    NoConflictingCommitCertificatesProperty(CoreSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE NoConflictingCommitCertificatesProperty(
                   CoreSpecAt(initialContext))
    <2>1. CoreSpecAt(initialContext) => []StrongInductiveInvariant
      BY CoreSpecAtAlwaysStrongInductiveInvariant
    <2>2. StrongInductiveInvariant
             => \A left, right \in commitQCs:
                  left.context = right.context
                    => left.subject = right.subject
      BY StrongInvariantImpliesCommitCertificateAgreement
    <2> QED BY <2>1, <2>2, PTL
       DEF NoConflictingCommitCertificatesProperty
  <1> QED BY <1>1

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

THEOREM RestartIncrementsSelectedGeneration ==
  \A node \in ValidatorIds:
    TypeInvariant /\ Restart(node)
      => generation'[node] = generation[node] + 1
BY Isa DEF TypeInvariant, Restart

THEOREM CrashAndRestartPreserveDurableSafety ==
  /\ CrashPreservesDurableProjection
  /\ RestartPreservesDurableProjection
  /\ PendingWritesAreUnacknowledged
  /\ (TypeInvariant => StaleGenerationRejected)
PROOF
  <1>1. CrashPreservesDurableProjection
    BY SMTT(120)
       DEF CrashPreservesDurableProjection, DurableProjection,
           DurableProjectionPrime, Crash
  <1>2. RestartPreservesDurableProjection
    BY SMTT(120)
       DEF RestartPreservesDurableProjection, DurableProjection,
           DurableProjectionPrime, Restart
  <1>3. PendingWritesAreUnacknowledged
    BY SMTT(120) DEF PendingWritesAreUnacknowledged, Crash
  <1>4. TypeInvariant => StaleGenerationRejected
    <2>1. ASSUME TypeInvariant,
                  NEW node \in ValidatorIds,
                  Restart(node)
           PROVE generation'[node] > generation[node]
      <3>1. generation'[node] = generation[node] + 1
        BY <2>1, RestartIncrementsSelectedGeneration
           DEF StrongInductiveInvariant, Safety
      <3>2. generation[node] \in Int
        BY <2>1 DEF TypeInvariant
      <3> QED BY <3>1, <3>2, SMT
    <2> QED BY <2>1 DEF StaleGenerationRejected
  <1> QED BY <1>1, <1>2, <1>3, <1>4

CrashRecoveryProperty(specification) ==
  /\ (specification => []CrashRecoveryStateInvariant)
  /\ CrashPreservesDurableProjection
  /\ RestartPreservesDurableProjection
  /\ PendingWritesAreUnacknowledged
  /\ (TypeInvariant => StaleGenerationRejected)

THEOREM CrashRecoveryObligation ==
  \A initialContext:
    CrashRecoveryProperty(CoreSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE CrashRecoveryProperty(CoreSpecAt(initialContext))
    <2>1. CoreSpecAt(initialContext) => []StrongInductiveInvariant
      BY CoreSpecAtAlwaysStrongInductiveInvariant
    <2>2. StrongInductiveInvariant => CrashRecoveryStateInvariant
      BY DEF StrongInductiveInvariant, Safety,
             CrashRecoveryStateInvariant
    <2>3. CoreSpecAt(initialContext) => []CrashRecoveryStateInvariant
      BY <2>1, <2>2, PTL
    <2> QED BY <2>3, CrashAndRestartPreserveDurableSafety
       DEF CrashRecoveryProperty
  <1> QED BY <1>1

THEOREM EpochBoundaryObligation ==
  Spec => []EpochBoundarySafety
PROOF
  <1>1. StrongInductiveInvariant => EpochBoundarySafety
    BY DEF StrongInductiveInvariant, EpochBoundarySafety
  <1> QED BY <1>1, SpecImpliesAlwaysStrongInductiveInvariant, PTL

=============================================================================
