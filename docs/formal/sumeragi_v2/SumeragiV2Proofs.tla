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
BY DEF QcValid, QcWireValid

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

THEOREM LockFunctionalUpdateAtKey ==
  \A domain, codomain, mapping, key, value:
    mapping \in [domain -> codomain] /\ key \in domain
      => [mapping EXCEPT ![key] = value][key] = value
BY Isa

THEOREM LockFunctionalUpdateAwayFromKey ==
  \A domain, codomain, mapping, key, value, other:
    mapping \in [domain -> codomain]
      /\ key \in domain
      /\ other \in domain
      /\ other # key
      => [mapping EXCEPT ![key] = value][other] = mapping[other]
BY Isa

THEOREM WellTypedTimeoutSelectorRankIsInteger ==
  \A tc:
    ModelConfiguration /\ TcWellTyped(tc) => TcHighRank(tc) \in Int
PROOF
  <1>1. ASSUME NEW tc, ModelConfiguration, TcWellTyped(tc)
         PROVE TcHighRank(tc) \in Int
    <2>1. Ranks \subseteq Int
      BY <1>1, ModelRanksAreIntegers
    <2>2. CASE MaximalTimeoutVotes(tc.votes) = {}
      <3>1. TcHighRank(tc) = NoRank
        BY <2>2 DEF TcHighRank, HighestTimeoutVote, EmptyTimeoutHigh
      <3> QED BY <3>1, SMT DEF NoRank
    <2>3. CASE MaximalTimeoutVotes(tc.votes) # {}
      <3>1. HighestTimeoutVote(tc.votes)
               \in MaximalTimeoutVotes(tc.votes)
        BY <2>3, Zenon DEF HighestTimeoutVote
      <3>2. HighestTimeoutVote(tc.votes) \in tc.votes
        BY <3>1 DEF MaximalTimeoutVotes
      <3>3. HighestTimeoutVote(tc.votes) \in TimeoutVoteRecordSet
        BY <1>1, <3>2, Isa DEF TcWellTyped
      <3>4. HighestTimeoutVote(tc.votes).highRank \in Ranks
        BY <3>3, Isa DEF TimeoutVoteRecordSet
      <3> QED BY <2>1, <3>4 DEF TcHighRank
    <2>4. MaximalTimeoutVotes(tc.votes) = {}
             \/ MaximalTimeoutVotes(tc.votes) # {}
      BY Isa
    <2> QED BY <2>2, <2>3, <2>4
  <1> QED BY <1>1

THEOREM PersistLockCommitIsLockMonotone ==
  \A request:
    TypeInvariant /\ PendingVoteWritesAuthorized
      /\ PersistLockCommit(request)
      => LockMonotonicityAction
PROOF
  <1>1. ASSUME NEW request,
                TypeInvariant,
                PendingVoteWritesAuthorized,
                PersistLockCommit(request)
         PROVE LockMonotonicityAction
    <2>1. request \in pendingLockCommit
      BY <1>1 DEF PersistLockCommit
    <2>2. /\ pendingLockCommit \subseteq LockCommitWalSet
          /\ lockRank \in [ValidatorIds -> Ranks]
          /\ lockSubject \in [ValidatorIds -> SubjectOrNone]
      BY <1>1 DEF TypeInvariant
    <2>3. /\ request.node \in ValidatorIds
          /\ request.qc \in QcRecordSet
      BY <2>1, <2>2, Isa DEF LockCommitWalSet
    <2>4. request.qc.view \in Views
      BY <2>3 DEF QcRecordSet
    <2>5. request.qc.view \in Ranks
      BY <2>4, ViewsAreRanks
    <2>6. lockRank[request.node] \in Ranks
      BY <2>2, <2>3, FunctionValueHasCodomain
    <2>7. /\ request.qc.view >= lockRank[request.node]
          /\ (request.qc.view = lockRank[request.node]
                => request.qc.subject = lockSubject[request.node])
      BY <1>1, <2>1 DEF PendingVoteWritesAuthorized
    <2>8. CommitLockAllowed(
             LockValue(lockRank[request.node],
                       lockSubject[request.node]),
             request.qc)
      BY <2>7 DEF CommitLockAllowed, LockValue
    <2>9. /\ request.qc.view \in Int
          /\ lockRank[request.node] \in Int
      BY <1>1, <2>5, <2>6, ModelRanksAreIntegers, Isa
         DEF TypeInvariant
    <2>10. LockMonotone(
             LockValue(lockRank[request.node],
                       lockSubject[request.node]),
             CommitLockResult(request.qc))
      BY <2>8, <2>9, CommitPersistenceAdvancesLockMonotonically
         DEF LockValue
    <2>11. ASSUME NEW node \in ValidatorIds
           PROVE /\ context' = context
                       => lockRank'[node] >= lockRank[node]
                 /\ (context' = context
                      /\ lockSubject'[node] # lockSubject[node]
                       => lockRank'[node] > lockRank[node])
      <3>1. /\ context' = context
            /\ lockRank' =
                 [lockRank EXCEPT ![request.node] = request.qc.view]
            /\ lockSubject' =
                 [lockSubject EXCEPT
                    ![request.node] = request.qc.subject]
        BY <1>1 DEF PersistLockCommit
      <3>2. CASE node = request.node
        <4>1. /\ lockRank'[node] = request.qc.view
              /\ lockSubject'[node] = request.qc.subject
          BY <2>2, <2>3, <3>1, <3>2,
             LockFunctionalUpdateAtKey
        <4> QED BY <2>10, <3>1, <3>2, <4>1, Isa
           DEF LockMonotone, CommitLockResult, LockValue
      <3>3. CASE node # request.node
        <4>1. /\ lockRank'[node] = lockRank[node]
              /\ lockSubject'[node] = lockSubject[node]
          BY <2>2, <2>3, <2>11, <3>1, <3>3,
             LockFunctionalUpdateAwayFromKey
        <4>2. lockRank[node] \in Int
          BY <1>1, <2>11, ModelRanksAreIntegers, Isa
             DEF TypeInvariant
        <4> QED BY <3>1, <3>3, <4>1, <4>2, SMT
      <3> QED BY <3>2, <3>3
    <2> QED BY <2>11 DEF LockMonotonicityAction
  <1> QED BY <1>1

THEOREM PersistInstallTCIsLockMonotone ==
  \A request:
    TypeInvariant /\ PersistInstallTC(request)
      => LockMonotonicityAction
PROOF
  <1>1. ASSUME NEW request, TypeInvariant, PersistInstallTC(request)
         PROVE LockMonotonicityAction
    <2>1. request \in pendingInstallTC
      BY <1>1 DEF PersistInstallTC
    <2>2. /\ lockRank \in [ValidatorIds -> Ranks]
          /\ lockSubject \in [ValidatorIds -> SubjectOrNone]
          /\ ModelConfiguration
      BY <1>1 DEF TypeInvariant
    <2>3. request \in InstallTcWalSet
      BY <1>1, <2>1, Isa DEF TypeInvariant
    <2>4. /\ request.node \in ValidatorIds
          /\ request.tc \in TcRecordSet
      BY <2>3, Isa DEF InstallTcWalSet
    <2>5. TcWellTyped(request.tc)
      BY <2>4, Isa DEF TcWellTyped, TcRecordSet
    <2>6. TcHighRank(request.tc) \in Int
      BY <2>2, <2>5, WellTypedTimeoutSelectorRankIsInteger
    <2>7. lockRank[request.node] \in Int
      BY <1>1, <2>4, ModelRanksAreIntegers, Isa DEF TypeInvariant
    <2>8. LockMonotone(
             LockValue(lockRank[request.node],
                       lockSubject[request.node]),
             InstallHighLock(
               LockValue(lockRank[request.node],
                         lockSubject[request.node]),
               TcHighRank(request.tc),
               TcHighSubject(request.tc)))
      BY <2>6, <2>7, TimeoutInstallationAdvancesLockMonotonically
         DEF LockValue
    <2>9. ASSUME NEW node \in ValidatorIds
           PROVE /\ context' = context
                       => lockRank'[node] >= lockRank[node]
                 /\ (context' = context
                      /\ lockSubject'[node] # lockSubject[node]
                       => lockRank'[node] > lockRank[node])
      <3>1. /\ context' = context
            /\ lockRank' =
                 [lockRank EXCEPT ![request.node] =
                    IF TcHighRank(request.tc) > lockRank[request.node]
                    THEN TcHighRank(request.tc) ELSE @]
            /\ lockSubject' =
                 [lockSubject EXCEPT ![request.node] =
                    IF TcHighRank(request.tc) > lockRank[request.node]
                    THEN TcHighSubject(request.tc) ELSE @]
        BY <1>1 DEF PersistInstallTC
      <3>2. CASE node = request.node
        <4>1. /\ lockRank'[node] =
                     IF TcHighRank(request.tc) > lockRank[request.node]
                     THEN TcHighRank(request.tc)
                     ELSE lockRank[request.node]
              /\ lockSubject'[node] =
                     IF TcHighRank(request.tc) > lockRank[request.node]
                     THEN TcHighSubject(request.tc)
                     ELSE lockSubject[request.node]
          BY <2>2, <2>4, <3>1, <3>2,
             LockFunctionalUpdateAtKey
        <4>2. LockValue(lockRank'[node], lockSubject'[node]) =
                 InstallHighLock(
                   LockValue(lockRank[request.node],
                             lockSubject[request.node]),
                   TcHighRank(request.tc),
                   TcHighSubject(request.tc))
          BY <4>1 DEF InstallHighLock, LockValue
        <4>3. LockMonotone(
                 LockValue(lockRank[node], lockSubject[node]),
                 LockValue(lockRank'[node], lockSubject'[node]))
          BY <2>8, <3>2, <4>2, Isa
        <4>4. lockRank'[node] >= lockRank[node]
          BY <4>3 DEF LockMonotone, LockValue
        <4>5. lockSubject'[node] # lockSubject[node]
                   => lockRank'[node] > lockRank[node]
          BY <4>3 DEF LockMonotone, LockValue
        <4> QED BY <3>1, <4>4, <4>5
      <3>3. CASE node # request.node
        <4>1. /\ lockRank'[node] = lockRank[node]
              /\ lockSubject'[node] = lockSubject[node]
          BY <2>2, <2>4, <2>9, <3>1, <3>3,
             LockFunctionalUpdateAwayFromKey
        <4>2. lockRank[node] \in Int
          BY <1>1, <2>9, ModelRanksAreIntegers, Isa DEF TypeInvariant
        <4> QED BY <3>1, <3>3, <4>1, <4>2, SMT
      <3> QED BY <3>2, <3>3
    <2> QED BY <2>9 DEF LockMonotonicityAction
  <1> QED BY <1>1

UnchangedContextAndLocks ==
  UNCHANGED <<context, lockRank, lockSubject>>

THEOREM UnchangedContextAndLocksIsLockMonotone ==
  TypeInvariant /\ UnchangedContextAndLocks => LockMonotonicityAction
PROOF
  <1>1. ASSUME TypeInvariant, UnchangedContextAndLocks
         PROVE LockMonotonicityAction
    <2>1. /\ context' = context
          /\ lockRank' = lockRank
          /\ lockSubject' = lockSubject
      BY <1>1, Isa DEF UnchangedContextAndLocks
    <2>2. ASSUME NEW node \in ValidatorIds
           PROVE /\ context' = context
                       => lockRank'[node] >= lockRank[node]
                 /\ (context' = context
                      /\ lockSubject'[node] # lockSubject[node]
                       => lockRank'[node] > lockRank[node])
      <3>1. lockRank[node] \in Int
        BY <1>1, <2>2, ModelRanksAreIntegers, Isa
           DEF TypeInvariant
      <3> QED BY <2>1, <3>1, SMT
    <2> QED BY <2>2 DEF LockMonotonicityAction
  <1> QED BY <1>1

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
       FetchBody(node, proposal) \/ RebindRetainedBody(node, proposal)
  \/ \E node \in ValidatorIds, roundView \in Views, subject \in Subjects:
       StoreBody(node, roundView, subject)
  \/ \E node \in ValidatorIds, proposal \in SeenProposalValues:
       ValidateBody(node, proposal) \/ RejectBody(node, proposal)
  \/ \E node \in ValidatorIds, qc \in DecisionQcValues:
       ValidateDecidedBody(node, qc)
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
  \/ \E node \in ValidatorIds, qc \in LockCommitQcValues:
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
       DeliverProposal, FetchBody, RebindRetainedBody, StoreBody,
       ValidateBody, ValidateDecidedBody, RejectBody,
       BeginPrepare, PersistPrepare,
       CompleteVoteSignature, ByzantineBroadcastVote, DeliverVote,
       FormPrepareQC, DeliverQC, BeginObservePrepare,
       PersistObservePrepare, BeginLockCommit, FormCommitQC,
       BeginDecision, PersistDecision, BeginTimeout, PersistTimeout,
       CompleteTimeoutSignature, ByzantineBroadcastTimeout,
       DeliverTimeout, FormTC, DeliverTC, BeginInstallTC,
       FetchCertifiedBody, ApplyDecision, Crash, Restart, ResumeProposal,
       ResumeVote, ResumeTimeout, DropProposal

(***************************************************************************
Only the two durable receipt consumers can change `decisions` or `applied`.
Keeping this classification at the Core proof boundary lets parameterized
chain instances reason about receipt handoff without expanding the complete
asynchronous command executor.
***************************************************************************)
THEOREM NextDurableReceiptActionClassification ==
  Next
    => \/ UNCHANGED <<decisions, applied>>
       \/ (\E request \in pendingDecision: PersistDecision(request))
       \/ (\E node \in ValidatorIds, qc \in DecisionQcValues:
             ApplyDecision(node, qc))
BY IsaM("blast")
   DEF Next, SetGST, AssembleLocalBody, BeginLocalProposal, PersistProposal,
       CompleteProposalSignature, ByzantineBroadcastProposal,
       DeliverProposal, FetchBody, RebindRetainedBody, StoreBody, ValidateBody,
       ValidateDecidedBody, RejectBody, BeginPrepare, PersistPrepare,
       CompleteVoteSignature, ByzantineBroadcastVote, DeliverVote,
       FormPrepareQC, DeliverQC, BeginObservePrepare,
       PersistObservePrepare, BeginLockCommit, PersistLockCommit,
       FormCommitQC, BeginDecision, BeginTimeout, PersistTimeout,
       CompleteTimeoutSignature, ByzantineBroadcastTimeout,
       DeliverTimeout, FormTC, DeliverTC, BeginInstallTC, PersistInstallTC,
       FetchCertifiedBody, Crash, Restart, ResumeProposal, ResumeVote,
       ResumeTimeout, DropProposal

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
      BodyHeldBy(durableBodies, signer, qc.context, qc.view, qc.subject)

CommitCertificateAvailability ==
  \A qc \in commitQCs:
    \E signer \in qc.signers \cap Honest:
      BodyHeldBy(durableBodies, signer, qc.context, qc.view, qc.subject)

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

PotentialCommitVotes(certificateContext, roundView, subject) ==
  {vote \in commitIntents:
    /\ vote.context = certificateContext
    /\ vote.view = roundView
    /\ vote.phase = "Commit"
    /\ vote.subject = subject}

PotentialCommitSigners(certificateContext, roundView, subject) ==
  {vote.signer:
    vote \in PotentialCommitVotes(
      certificateContext, roundView, subject)}

InstalledTcAuthorizedPotentialCommitIntersection(tc, protectedView, subject) ==
  \E timeoutVote \in tc.votes,
      commitVote \in PotentialCommitVotes(
        tc.context, protectedView, subject):
    /\ timeoutVote.signer \in Honest
    /\ commitVote.signer = timeoutVote.signer
    /\ timeoutVote.context = tc.context
    /\ timeoutVote.view = tc.view
    /\ ~TimeoutVoteStrictlyProtectsCommit(timeoutVote, commitVote)
    /\ InstalledTcAuthorizesCommitVote(commitVote)

TCProtectsOrInstalledTcAuthorizesPotentialCommit(tc) ==
  \A protectedView \in 0..tc.view, subject \in Subjects:
    DualQuorum(tc.context.epoch,
      PotentialCommitSigners(tc.context, protectedView, subject))
      => \/ TCProtectsViewSubject(tc, protectedView, subject)
         \/ InstalledTcAuthorizedPotentialCommitIntersection(
              tc, protectedView, subject)

THEOREM StrongInvariantImpliesTimeoutProtectionAlternative ==
  StrongInductiveInvariant
    => \A tc \in formedTCs:
         TCProtectsOrInstalledTcAuthorizesPotentialCommit(tc)
PROOF
  <1>1. ASSUME StrongInductiveInvariant,
              NEW tc \in formedTCs
         PROVE TCProtectsOrInstalledTcAuthorizesPotentialCommit(tc)
    <2>1. ASSUME NEW protectedView \in 0..tc.view,
                NEW subject \in Subjects,
                DualQuorum(
                  tc.context.epoch,
                  PotentialCommitSigners(
                    tc.context, protectedView, subject))
           PROVE \/ TCProtectsViewSubject(
                        tc, protectedView, subject)
                 \/ InstalledTcAuthorizedPotentialCommitIntersection(
                      tc, protectedView, subject)
      <3> DEFINE CommitSigners ==
             PotentialCommitSigners(
               tc.context, protectedView, subject)
      <3> DEFINE TimeoutSigners == TimeoutSignerSet(tc.votes)
      <3>1. /\ QuorumConfiguration
            /\ tc.context.epoch \in Epochs
            /\ DualQuorum(tc.context.epoch, CommitSigners)
            /\ DualQuorum(tc.context.epoch, TimeoutSigners)
        BY <1>1, <2>1
           DEF StrongInductiveInvariant, Safety, TypeInvariant,
               ModelConfiguration, ReducerProvenanceInvariant,
               FormedTimeoutCertificatesSound,
               CommitSigners, TimeoutSigners
      <3>2. /\ CommitSigners
                   \in SUBSET VotingRoster(tc.context.epoch)
            /\ TimeoutSigners
                   \in SUBSET VotingRoster(tc.context.epoch)
        BY <3>1 DEF DualQuorum, CountQuorum
      <3>3. DualQuorumIntersectionHasHonest
        BY <3>1, DualQuorumHonestIntersection
      <3>4. (CommitSigners \cap TimeoutSigners \cap Honest) # {}
        BY <3>1, <3>2, <3>3
           DEF DualQuorumIntersectionHasHonest
      <3>5. PICK signer
                    \in CommitSigners \cap TimeoutSigners \cap Honest:
               TRUE
        BY <3>4
      <3>6. PICK commitVote
                    \in PotentialCommitVotes(
                         tc.context, protectedView, subject):
               commitVote.signer = signer
        BY <3>5 DEF CommitSigners, PotentialCommitSigners
      <3>7. PICK timeoutVote \in tc.votes:
               timeoutVote.signer = signer
        BY <3>5 DEF TimeoutSigners, TimeoutSignerSet
      <3>8. /\ timeoutVote.signer \in Honest
            /\ timeoutVote.context = tc.context
            /\ timeoutVote.view = tc.view
            /\ timeoutVote \in timeoutIntents
            /\ TCMaximumProtectsReports(tc)
        BY <1>1, <3>5, <3>7
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               FormedTimeoutCertificatesSound,
               TimeoutVotesBindCertificate
      <3>9. /\ commitVote \in commitIntents
            /\ commitVote.signer = timeoutVote.signer
            /\ commitVote.context = tc.context
            /\ commitVote.view = protectedView
            /\ commitVote.phase = "Commit"
            /\ commitVote.subject = subject
        BY <3>5, <3>6, <3>7 DEF PotentialCommitVotes
      <3>10. TimeoutVoteProtectsCommitSet(
                timeoutVote, commitIntents)
        BY <1>1, <3>8
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               DurableTimeoutsProtectCommits,
               TimeoutIntentProtectsCommits
      <3>11. \/ TimeoutVoteStrictlyProtectsCommit(
                    timeoutVote, commitVote)
              \/ InstalledTcAuthorizesCommitVote(commitVote)
        BY <2>1, <3>8, <3>9, <3>10, SMT
           DEF TimeoutVoteProtectsCommitSet
      <3>12. CASE TimeoutVoteStrictlyProtectsCommit(
                     timeoutVote, commitVote)
        <4>1. HighestTimeoutVote(tc.votes) \in tc.votes
          BY <1>1, StrongInvariantImpliesTimeoutCertificateSelectorsSound
             DEF TimeoutCertificateSelectorsSound
        <4>2. /\ protectedView \in Int
              /\ timeoutVote.highRank \in Int
              /\ TcHighRank(tc) \in Int
          <5>1. /\ timeoutVote.highRank \in Ranks
                /\ HighestTimeoutVote(tc.votes).highRank \in Ranks
            BY <1>1, <3>7, <4>1
               DEF StrongInductiveInvariant,
                   ReducerProvenanceInvariant,
                   FormedTimeoutCertificatesSound
          <5>2. ViewDomain \subseteq Nat
            BY <1>1
               DEF StrongInductiveInvariant, Safety, TypeInvariant,
                   ModelConfiguration
          <5>3. /\ Ranks = {NoRank} \cup ViewDomain
                /\ NoRank = -1
            BY DEF Ranks, NoRank, Views
          <5> QED BY <2>1, <5>1, <5>2, <5>3, SMT DEF TcHighRank
        <4>3. TCProtectsViewSubject(tc, protectedView, subject)
          BY <2>1, <3>8, <3>9, <3>12, <4>2, SMT
             DEF TCMaximumProtectsReports,
                 TimeoutVoteStrictlyProtectsCommit,
                 TCProtectsViewSubject
        <4> QED BY <4>3
      <3>13. CASE ~TimeoutVoteStrictlyProtectsCommit(
                     timeoutVote, commitVote)
        <4>1. InstalledTcAuthorizesCommitVote(commitVote)
          BY <3>11, <3>13
        <4>2. InstalledTcAuthorizedPotentialCommitIntersection(
                 tc, protectedView, subject)
          BY <3>6, <3>7, <3>8, <3>9, <3>13, <4>1
             DEF InstalledTcAuthorizedPotentialCommitIntersection
        <4> QED BY <4>2
      <3> QED BY <3>12, <3>13
    <2> QED BY <2>1
         DEF TCProtectsOrInstalledTcAuthorizesPotentialCommit
  <1> QED BY <1>1

(***************************************************************************
The strict grouped-timeout kernel remains proved for Commit intents that
already existed when the timeout votes were made.  A node may instead learn
an exact historical Prepare lock from an installed TC after its own lower-high
timeout, validate that body, and only then persist Commit intent.  In that
case the target TC's selected high need not directly protect the late-created
Commit.  Safety is retained because dual-quorum intersection supplies an
honest timeout/Commit signer and the durable historical invariant supplies
that exact Commit's installed-TC authorization.  The authorizing installed TC
need not be later than the formed TC quantified by the property.
***************************************************************************)
HistoricalTcLockedCommitAuthorizationProperty(specification) ==
  specification => []HistoricalTcLockedCommitAuthorizationInvariant

THEOREM HistoricalTcLockedCommitAuthorizationObligation ==
  \A initialContext:
    HistoricalTcLockedCommitAuthorizationProperty(
      CoreSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE HistoricalTcLockedCommitAuthorizationProperty(
                 CoreSpecAt(initialContext))
    <2>1. CoreSpecAt(initialContext) => []StrongInductiveInvariant
      BY CoreSpecAtAlwaysStrongInductiveInvariant
    <2>2. StrongInductiveInvariant
             => HistoricalTcLockedCommitAuthorizationInvariant
      BY ReducerProvenanceImpliesHistoricalTcLockedCommitAuthorization
         DEF StrongInductiveInvariant
    <2> QED BY <2>1, <2>2, PTL
         DEF HistoricalTcLockedCommitAuthorizationProperty
  <1> QED BY <1>1

TimeoutProtectionProperty(specification) ==
  specification
    => [](\A tc \in formedTCs:
          TCProtectsOrInstalledTcAuthorizesPotentialCommit(tc))

THEOREM TimeoutProtectionObligation ==
  \A initialContext:
    TimeoutProtectionProperty(CoreSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE TimeoutProtectionProperty(CoreSpecAt(initialContext))
    <2>1. CoreSpecAt(initialContext) => []StrongInductiveInvariant
      BY CoreSpecAtAlwaysStrongInductiveInvariant
    <2>2. StrongInductiveInvariant
             => \A tc \in formedTCs:
                  TCProtectsOrInstalledTcAuthorizesPotentialCommit(tc)
      BY StrongInvariantImpliesTimeoutProtectionAlternative
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
        BY <2>1 DEF TypeInvariant, Generations
      <3> QED BY <3>1, <3>2, SMT
    <2> QED BY <2>1 DEF StaleGenerationRejected
  <1> QED BY <1>1, <1>2, <1>3, <1>4

CrashRecoveryProperty(specification) ==
  /\ (specification => []CrashRecoveryStateInvariant)
  /\ (specification => [][CrashPreservesDurableProjection]_vars)
  /\ (specification => [][RestartPreservesDurableProjection]_vars)
  /\ (specification => [][PendingWritesAreUnacknowledged]_vars)
  /\ (specification =>
        [][TypeInvariant => StaleGenerationRejected]_vars)

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
    <2>4. CoreSpecAt(initialContext)
            => /\ [][CrashPreservesDurableProjection]_vars
               /\ [][RestartPreservesDurableProjection]_vars
               /\ [][PendingWritesAreUnacknowledged]_vars
               /\ [][TypeInvariant => StaleGenerationRejected]_vars
      BY CrashAndRestartPreserveDurableSafety, PTL
    <2> QED BY <2>3, <2>4
       DEF CrashRecoveryProperty
  <1> QED BY <1>1

THEOREM EpochBoundaryObligation ==
  Spec => []EpochBoundarySafety
PROOF
  <1>1. StrongInductiveInvariant => EpochBoundarySafety
    BY DEF StrongInductiveInvariant, EpochBoundarySafety
  <1> QED BY <1>1, SpecImpliesAlwaysStrongInductiveInvariant, PTL

=============================================================================
