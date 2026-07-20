---- MODULE SumeragiV2TimeoutViewInvariant ----
EXTENDS SumeragiV2Proofs, FunctionTheorems

(***************************************************************************
Reachable timeout view-frontier invariant for the authoritative Core relation.

This module intentionally contains no fairness specification.  It proves only
that the highest PrepareQC and every pending observe-Prepare WAL snapshot stay
at or below the owning validator's installed view.  The asynchronous timeout
proof imports this safety fact and must derive all service from AsyncFairnessAt.
***************************************************************************)

(***************************************************************************
View-frontier strengthening needed by timeout wire authorization.  The WAL
snapshot bound is explicit because PersistObservePrepare may run after its
BeginObservePrepare guard was checked.  Together these facts ensure a local
timeout's certified high reference never outruns its timeout view.
***************************************************************************)

HighestNotAheadOfView ==
  \A node \in ValidatorIds:
    highestRank[node] <= nodeView[node]

PendingObserveViewBound ==
  \A request \in pendingObservePrepare:
    request.qc.view <= nodeView[request.node]

TimeoutViewFrontierShapeInvariant ==
  /\ HighestNotAheadOfView
  /\ PendingObserveViewBound

StrongTimeoutViewFrontierInvariant ==
  /\ StrongInductiveInvariant
  /\ TimeoutViewFrontierShapeInvariant

TimeoutViewFrontierMutationStep ==
  \/ \E node \in ValidatorIds, qc \in ReceivedQcValues:
       BeginObservePrepare(node, qc)
  \/ \E request \in pendingObservePrepare:
       PersistObservePrepare(request)
  \/ \E request \in pendingLockCommit:
       PersistLockCommit(request)
  \/ \E request \in pendingInstallTC:
       PersistInstallTC(request)
  \/ \E node \in ValidatorIds: Crash(node)

TimeoutViewFrontierProposalStableStep ==
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
  \/ \E node \in ValidatorIds, qc \in prepareQCs:
       ValidateLockedBody(node, qc)

TimeoutViewFrontierVoteStableStep ==
  \/ \E node \in ValidatorIds, proposal \in SeenProposalValues:
       BeginPrepare(node, proposal)
  \/ \E request \in pendingPrepare: PersistPrepare(request)
  \/ \E request \in signVotes: CompleteVoteSignature(request)
  \/ \E signer \in ValidatorIds, roundView \in Views,
       phase \in Phases, subject \in Subjects:
       ByzantineBroadcastVote(signer, roundView, phase, subject)
  \/ \E envelope \in voteNetwork: DeliverVote(envelope)
  \/ \E node \in ValidatorIds, roundView \in Views,
       subject \in Subjects: FormPrepareQC(node, roundView, subject)
  \/ \E envelope \in qcNetwork: DeliverQC(envelope)
  \/ \E node \in ValidatorIds, qc \in LockCommitQcValues:
       BeginLockCommit(node, qc)
  \/ \E node \in ValidatorIds, roundView \in Views,
       subject \in Subjects: FormCommitQC(node, roundView, subject)
  \/ \E node \in ValidatorIds, qc \in ReceivedQcValues:
       BeginDecision(node, qc)
  \/ \E request \in pendingDecision: PersistDecision(request)

TimeoutViewFrontierTimeoutStableStep ==
  \/ \E node \in ValidatorIds: BeginTimeout(node)
  \/ \E request \in pendingTimeout: PersistTimeout(request)
  \/ \E request \in signTimeouts: CompleteTimeoutSignature(request)
  \/ \E signer \in ValidatorIds, roundView \in Views,
       highRank \in Ranks, highSubject \in SubjectOrNone:
       ByzantineBroadcastTimeout(signer, roundView, highRank, highSubject)
  \/ \E envelope \in timeoutNetwork: DeliverTimeout(envelope)
  \/ \E node \in ValidatorIds, roundView \in Views:
       FormTC(node, roundView)
  \/ \E envelope \in tcNetwork: DeliverTC(envelope)
  \/ \E node \in ValidatorIds, tc \in ReceivedTcValues:
       BeginInstallTC(node, tc)

TimeoutViewFrontierRecoveryStableStep ==
  \/ \E node \in ValidatorIds,
       qc \in DecisionQcValues \cup prepareQCs:
       FetchCertifiedBody(node, qc)
  \/ \E node \in ValidatorIds, qc \in DecisionQcValues:
       ApplyDecision(node, qc)
  \/ \E node \in ValidatorIds: Restart(node)
  \/ \E node \in ValidatorIds, proposal \in proposalIntents:
       ResumeProposal(node, proposal)
  \/ \E node \in ValidatorIds, vote \in prepareIntents \cup commitIntents:
       ResumeVote(node, vote)
  \/ \E node \in ValidatorIds, vote \in timeoutIntents:
       ResumeTimeout(node, vote)
  \/ \E envelope \in proposalNetwork: DropProposal(envelope)

TimeoutViewFrontierStableStep ==
  \/ TimeoutViewFrontierProposalStableStep
  \/ TimeoutViewFrontierVoteStableStep
  \/ TimeoutViewFrontierTimeoutStableStep
  \/ TimeoutViewFrontierRecoveryStableStep

THEOREM UnchangedTimeoutViewFrontierPreservesShape ==
  (TimeoutViewFrontierShapeInvariant
    /\ UNCHANGED <<nodeView, highestRank, pendingObservePrepare>>)
    => TimeoutViewFrontierShapeInvariant'
BY DEF TimeoutViewFrontierShapeInvariant,
       HighestNotAheadOfView, PendingObserveViewBound

THEOREM BeginObservePreparePreservesTimeoutViewFrontier ==
  \A node \in ValidatorIds, qc \in ReceivedQcValues:
    (TimeoutViewFrontierShapeInvariant
      /\ BeginObservePrepare(node, qc))
      => TimeoutViewFrontierShapeInvariant'
BY Isa
   DEF TimeoutViewFrontierShapeInvariant,
       HighestNotAheadOfView, PendingObserveViewBound,
       BeginObservePrepare, ObservePrepareWal

THEOREM PersistObservePreparePreservesTimeoutViewFrontier ==
  \A request \in pendingObservePrepare:
    (StrongInductiveInvariant
      /\ TimeoutViewFrontierShapeInvariant
      /\ PersistObservePrepare(request))
      => TimeoutViewFrontierShapeInvariant'
PROOF
  <1>1. ASSUME NEW request \in pendingObservePrepare,
                StrongInductiveInvariant,
                TimeoutViewFrontierShapeInvariant,
                PersistObservePrepare(request)
         PROVE TimeoutViewFrontierShapeInvariant'
    <2>1. request.node \in ValidatorIds
      BY <1>1
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             ObservePrepareWalSet
    <2>2. request.qc.view <= nodeView[request.node]
      BY <1>1
         DEF TimeoutViewFrontierShapeInvariant,
             PendingObserveViewBound
    <2>3. /\ nodeView' = nodeView
           /\ highestRank' =
                [highestRank EXCEPT
                   ![request.node] = request.qc.view]
           /\ pendingObservePrepare' =
                pendingObservePrepare \ {request}
      BY <1>1 DEF PersistObservePrepare
    <2>4. /\ DOMAIN nodeView = ValidatorIds
           /\ DOMAIN highestRank = ValidatorIds
      BY <1>1, Isa
         DEF StrongInductiveInvariant, Safety, TypeInvariant
    <2>5. \A node \in ValidatorIds:
             highestRank'[node] <= nodeView'[node]
      <3>1. ASSUME NEW node \in ValidatorIds
             PROVE highestRank'[node] <= nodeView'[node]
        <4>1. highestRank[node] <= nodeView[node]
          BY <1>1, <3>1
             DEF TimeoutViewFrontierShapeInvariant,
                 HighestNotAheadOfView
        <4>2. CASE node = request.node
          BY <2>1, <2>2, <2>3, <2>4, <4>2, Isa
        <4>3. CASE node # request.node
          BY <2>3, <2>4, <3>1, <4>1, <4>3, Isa
        <4> QED BY <4>2, <4>3
      <3> QED BY <3>1
    <2>6. \A pending \in pendingObservePrepare':
             pending.qc.view <= nodeView'[pending.node]
      <3>1. ASSUME NEW pending \in pendingObservePrepare'
             PROVE pending.qc.view <= nodeView'[pending.node]
        <4>1. pending \in pendingObservePrepare
          BY <2>3, <3>1
        <4>2. pending.qc.view <= nodeView[pending.node]
          BY <1>1, <4>1
             DEF TimeoutViewFrontierShapeInvariant,
                 PendingObserveViewBound
        <4> QED BY <2>3, <4>2
      <3> QED BY <3>1
    <2> QED BY <2>5, <2>6
       DEF TimeoutViewFrontierShapeInvariant,
           HighestNotAheadOfView, PendingObserveViewBound
  <1> QED BY <1>1

THEOREM PersistLockCommitPreservesTimeoutViewFrontier ==
  \A request \in pendingLockCommit:
    (StrongInductiveInvariant
      /\ TimeoutViewFrontierShapeInvariant
      /\ PersistLockCommit(request))
      => TimeoutViewFrontierShapeInvariant'
PROOF
  <1>1. ASSUME NEW request \in pendingLockCommit,
                StrongInductiveInvariant,
                TimeoutViewFrontierShapeInvariant,
                PersistLockCommit(request)
         PROVE TimeoutViewFrontierShapeInvariant'
    <2>1. request.node \in ValidatorIds
      BY <1>1
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             LockCommitWalSet
    <2>2. request.qc.view = nodeView[request.node]
      BY <1>1
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             PendingVoteWritesAuthorized
    <2>3. /\ nodeView' = nodeView
           /\ highestRank' =
                [highestRank EXCEPT
                   ![request.node] =
                     IF request.qc.view > highestRank[request.node]
                     THEN request.qc.view
                     ELSE highestRank[request.node]]
           /\ pendingObservePrepare' = pendingObservePrepare
      BY <1>1 DEF PersistLockCommit
    <2>4. /\ DOMAIN nodeView = ValidatorIds
           /\ DOMAIN highestRank = ValidatorIds
      BY <1>1, Isa
         DEF StrongInductiveInvariant, Safety, TypeInvariant
    <2>5. /\ ModelConfiguration
           /\ request.qc.view \in Views
      BY <1>1
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             LockCommitWalSet, QcRecordSet
    <2>6. request.qc.view \in Nat
      BY <2>5, SMT DEF ModelConfiguration, Views
    <2>7. \A node \in ValidatorIds:
             highestRank'[node] <= nodeView'[node]
      <3>1. ASSUME NEW node \in ValidatorIds
             PROVE highestRank'[node] <= nodeView'[node]
        <4>1. highestRank[node] <= nodeView[node]
          BY <1>1, <3>1
             DEF TimeoutViewFrontierShapeInvariant,
                 HighestNotAheadOfView
        <4>2. CASE node = request.node
          <5>1. /\ highestRank'[node] =
                       (IF request.qc.view > highestRank[node]
                        THEN request.qc.view
                        ELSE highestRank[node])
                 /\ nodeView'[node] = nodeView[node]
            BY <2>1, <2>3, <2>4, <4>2, Isa
          <5>2. CASE request.qc.view > highestRank[node]
            BY <2>2, <2>6, <4>2, <5>1, <5>2, SMT
          <5>3. CASE ~(request.qc.view > highestRank[node])
            BY <4>1, <5>1, <5>3
          <5> QED BY <5>2, <5>3
        <4>3. CASE node # request.node
          BY <2>3, <2>4, <3>1, <4>1, <4>3, Isa
        <4> QED BY <4>2, <4>3
      <3> QED BY <3>1
    <2>8. \A pending \in pendingObservePrepare':
             pending.qc.view <= nodeView'[pending.node]
      BY <1>1, <2>3
         DEF TimeoutViewFrontierShapeInvariant,
             PendingObserveViewBound
    <2> QED BY <2>7, <2>8
       DEF TimeoutViewFrontierShapeInvariant,
           HighestNotAheadOfView, PendingObserveViewBound
  <1> QED BY <1>1

THEOREM ValidInstallSelectedRankDoesNotExceedTcView ==
  \A request \in pendingInstallTC:
    (StrongInductiveInvariant /\ PersistInstallTC(request))
      => /\ TcHighRank(request.tc) \in Ranks
         /\ TcHighRank(request.tc) <= request.tc.view
PROOF
  <1>1. ASSUME NEW request \in pendingInstallTC,
                StrongInductiveInvariant,
                PersistInstallTC(request)
         PROVE /\ TcHighRank(request.tc) \in Ranks
               /\ TcHighRank(request.tc) <= request.tc.view
    <2>1. /\ ModelConfiguration
           /\ TCValid(request.tc)
           /\ request.tc.votes # {}
      BY <1>1
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             ReducerProvenanceInvariant,
             PendingCertificateWritesAuthorized
    <2>2. HighestTimeoutVote(request.tc.votes)
             \in request.tc.votes
      BY <2>1, ValidTimeoutCertificateSelectsMember
    <2>3. /\ HighestTimeoutVote(request.tc.votes).highRank \in Ranks
           /\ HighestTimeoutVote(request.tc.votes).highRank
                <= request.tc.view
      BY <2>1, <2>2 DEF TCValid, TimeoutVoteRecordSet
    <2> QED BY <2>3 DEF TcHighRank
  <1> QED BY <1>1

THEOREM PersistInstallTCPreservesTimeoutViewFrontier ==
  \A request \in pendingInstallTC:
    (StrongInductiveInvariant
      /\ TimeoutViewFrontierShapeInvariant
      /\ PersistInstallTC(request))
      => TimeoutViewFrontierShapeInvariant'
PROOF
  <1>1. ASSUME NEW request \in pendingInstallTC,
                StrongInductiveInvariant,
                TimeoutViewFrontierShapeInvariant,
                PersistInstallTC(request)
         PROVE TimeoutViewFrontierShapeInvariant'
    <2>1. /\ TcHighRank(request.tc) \in Ranks
           /\ TcHighRank(request.tc) <= request.tc.view
      BY <1>1, ValidInstallSelectedRankDoesNotExceedTcView
    <2>2. /\ request.node \in ValidatorIds
           /\ request.tc.view \in Views
           /\ request.tc.view >= nodeView[request.node]
           /\ highestRank \in [ValidatorIds -> Ranks]
           /\ nodeView \in [ValidatorIds -> Views]
      BY <1>1
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             PersistInstallTC, InstallTcWalSet, TcRecordSet
    <2>3. /\ nodeView' =
                  [nodeView EXCEPT
                     ![request.node] = request.tc.view + 1]
           /\ highestRank' =
                  [highestRank EXCEPT
                     ![request.node] =
                       IF TcHighRank(request.tc)
                            > highestRank[request.node]
                       THEN TcHighRank(request.tc)
                       ELSE highestRank[request.node]]
           /\ pendingObservePrepare' = pendingObservePrepare
      BY <1>1 DEF PersistInstallTC
    <2>4. ModelConfiguration
      BY <1>1 DEF StrongInductiveInvariant, Safety, TypeInvariant
    <2>5. /\ DOMAIN nodeView = ValidatorIds
           /\ DOMAIN highestRank = ValidatorIds
      BY <2>2, Isa
    <2>6. \A node \in ValidatorIds:
             highestRank'[node] <= nodeView'[node]
      <3>1. ASSUME NEW node \in ValidatorIds
             PROVE highestRank'[node] <= nodeView'[node]
        <4>1. highestRank[node] <= nodeView[node]
          BY <1>1, <3>1
             DEF TimeoutViewFrontierShapeInvariant,
                 HighestNotAheadOfView
        <4>2. /\ highestRank[node] \in Ranks
               /\ nodeView[node] \in Views
               /\ TcHighRank(request.tc) \in Ranks
               /\ request.tc.view \in Views
          BY <2>1, <2>2, <3>1, Isa
        <4>3. /\ highestRank[node] \in Int
               /\ nodeView[node] \in Int
               /\ TcHighRank(request.tc) \in Int
               /\ request.tc.view \in Nat
          BY <2>4, <4>2, SMT DEF ModelConfiguration, Ranks, Views, NoRank
        <4>4. CASE node = request.node
          <5>1. /\ highestRank'[node] =
                       (IF TcHighRank(request.tc) > highestRank[node]
                        THEN TcHighRank(request.tc)
                        ELSE highestRank[node])
                 /\ nodeView'[node] = request.tc.view + 1
            BY <2>3, <2>5, <4>4, Isa
          <5>2. CASE TcHighRank(request.tc) > highestRank[node]
            BY <2>1, <4>3, <4>4, <5>1, <5>2, SMT
          <5>3. CASE ~(TcHighRank(request.tc) > highestRank[node])
            BY <2>2, <4>1, <4>3, <4>4, <5>1, <5>3, SMT
          <5> QED BY <5>2, <5>3
        <4>5. CASE node # request.node
          <5>1. /\ highestRank'[node] = highestRank[node]
                 /\ nodeView'[node] = nodeView[node]
            BY <2>3, <2>5, <3>1, <4>5, Isa
          <5> QED BY <4>1, <5>1
        <4> QED BY <4>4, <4>5
      <3> QED BY <3>1
    <2>7. \A pending \in pendingObservePrepare':
             pending.qc.view <= nodeView'[pending.node]
      <3>1. ASSUME NEW pending \in pendingObservePrepare'
             PROVE pending.qc.view <= nodeView'[pending.node]
        <4>1. /\ pending \in pendingObservePrepare
               /\ pending.qc.view <= nodeView[pending.node]
          BY <1>1, <2>3, <3>1
             DEF TimeoutViewFrontierShapeInvariant,
                 PendingObserveViewBound
        <4>2. pending \in ObservePrepareWalSet
          BY <1>1, <4>1
             DEF StrongInductiveInvariant, Safety, TypeInvariant
        <4>3. /\ pending.node \in ValidatorIds
               /\ pending.qc.view \in Views
               /\ nodeView[pending.node] \in Views
               /\ request.tc.view \in Views
          BY <2>2, <4>2, Isa DEF ObservePrepareWalSet, QcRecordSet
        <4>4. /\ pending.qc.view \in Nat
               /\ nodeView[pending.node] \in Nat
               /\ request.tc.view \in Nat
          BY <2>4, <4>3, SMT DEF ModelConfiguration, Views
        <4>5. CASE pending.node = request.node
          <5>1. nodeView'[pending.node] = request.tc.view + 1
            BY <2>3, <2>5, <4>3, <4>5, Isa
          <5>2. pending.qc.view <= request.tc.view
            BY <2>2, <4>1, <4>4, <4>5, SMT
          <5>3. request.tc.view < request.tc.view + 1
            BY <4>4, SMT
          <5> QED BY <4>4, <5>1, <5>2, <5>3, SMT
        <4>6. CASE pending.node # request.node
          <5>1. nodeView'[pending.node] = nodeView[pending.node]
            BY <2>3, <2>5, <4>3, <4>6, Isa
          <5> QED BY <4>1, <5>1
        <4> QED BY <4>5, <4>6
      <3> QED BY <3>1
    <2> QED BY <2>6, <2>7
       DEF TimeoutViewFrontierShapeInvariant,
           HighestNotAheadOfView, PendingObserveViewBound
  <1> QED BY <1>1

THEOREM CrashPreservesTimeoutViewFrontier ==
  \A node \in ValidatorIds:
    (TimeoutViewFrontierShapeInvariant /\ Crash(node))
      => TimeoutViewFrontierShapeInvariant'
BY Isa
   DEF TimeoutViewFrontierShapeInvariant,
       HighestNotAheadOfView, PendingObserveViewBound, Crash

THEOREM TimeoutViewFrontierMutationPreservesShape ==
  (StrongInductiveInvariant
    /\ TimeoutViewFrontierShapeInvariant
    /\ TimeoutViewFrontierMutationStep)
    => TimeoutViewFrontierShapeInvariant'
PROOF
  <1>1. ASSUME StrongInductiveInvariant,
                TimeoutViewFrontierShapeInvariant,
                TimeoutViewFrontierMutationStep
         PROVE TimeoutViewFrontierShapeInvariant'
    <2>1. CASE \E node \in ValidatorIds, qc \in LockCommitQcValues:
                   BeginObservePrepare(node, qc)
      BY <1>1, <2>1,
         BeginObservePreparePreservesTimeoutViewFrontier
    <2>2. CASE \E request \in pendingObservePrepare:
                   PersistObservePrepare(request)
      BY <1>1, <2>2,
         PersistObservePreparePreservesTimeoutViewFrontier
    <2>3. CASE \E request \in pendingLockCommit:
                   PersistLockCommit(request)
      BY <1>1, <2>3,
         PersistLockCommitPreservesTimeoutViewFrontier
    <2>4. CASE \E request \in pendingInstallTC:
                   PersistInstallTC(request)
      BY <1>1, <2>4,
         PersistInstallTCPreservesTimeoutViewFrontier
    <2>5. CASE \E node \in ValidatorIds: Crash(node)
      BY <1>1, <2>5, CrashPreservesTimeoutViewFrontier
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4, <2>5
         DEF TimeoutViewFrontierMutationStep
  <1> QED BY <1>1

THEOREM ProposalStableStepLeavesTimeoutViewFrontier ==
  TimeoutViewFrontierProposalStableStep
    => UNCHANGED <<nodeView, highestRank, pendingObservePrepare>>
BY IsaT(60)
   DEF TimeoutViewFrontierProposalStableStep,
       SetGST, AssembleLocalBody, BeginLocalProposal,
       PersistProposal, CompleteProposalSignature,
       ByzantineBroadcastProposal, DeliverProposal,
       FetchBody, RebindRetainedBody, StoreBody,
       ValidateBody, RejectBody, ValidateDecidedBody, ValidateLockedBody

THEOREM VoteStableStepLeavesTimeoutViewFrontier ==
  TimeoutViewFrontierVoteStableStep
    => UNCHANGED <<nodeView, highestRank, pendingObservePrepare>>
BY IsaT(60)
   DEF TimeoutViewFrontierVoteStableStep,
       BeginPrepare, PersistPrepare, CompleteVoteSignature,
       ByzantineBroadcastVote, DeliverVote, FormPrepareQC, DeliverQC,
       BeginLockCommit, FormCommitQC,
       BeginDecision, PersistDecision

THEOREM TimeoutStableStepLeavesTimeoutViewFrontier ==
  TimeoutViewFrontierTimeoutStableStep
    => UNCHANGED <<nodeView, highestRank, pendingObservePrepare>>
BY IsaT(60)
   DEF TimeoutViewFrontierTimeoutStableStep,
       BeginTimeout, PersistTimeout, CompleteTimeoutSignature,
       ByzantineBroadcastTimeout, DeliverTimeout, FormTC, DeliverTC,
       BeginInstallTC

THEOREM RecoveryStableStepLeavesTimeoutViewFrontier ==
  TimeoutViewFrontierRecoveryStableStep
    => UNCHANGED <<nodeView, highestRank, pendingObservePrepare>>
BY IsaT(60)
   DEF TimeoutViewFrontierRecoveryStableStep,
       FetchCertifiedBody, ApplyDecision, Restart,
       ResumeProposal, ResumeVote, ResumeTimeout, DropProposal

THEOREM StableStepLeavesTimeoutViewFrontier ==
  TimeoutViewFrontierStableStep
    => UNCHANGED <<nodeView, highestRank, pendingObservePrepare>>
BY ProposalStableStepLeavesTimeoutViewFrontier,
   VoteStableStepLeavesTimeoutViewFrontier,
   TimeoutStableStepLeavesTimeoutViewFrontier,
   RecoveryStableStepLeavesTimeoutViewFrontier
   DEF TimeoutViewFrontierStableStep

THEOREM NextSplitsTimeoutViewFrontierSteps ==
  Next
    => \/ TimeoutViewFrontierMutationStep
       \/ TimeoutViewFrontierStableStep
BY Isa
   DEF Next, TimeoutViewFrontierMutationStep,
       TimeoutViewFrontierStableStep,
       TimeoutViewFrontierProposalStableStep,
       TimeoutViewFrontierVoteStableStep,
       TimeoutViewFrontierTimeoutStableStep,
       TimeoutViewFrontierRecoveryStableStep

THEOREM NextEitherMutatesTimeoutViewFrontierOrLeavesIt ==
  Next
    => \/ TimeoutViewFrontierMutationStep
       \/ UNCHANGED <<nodeView, highestRank, pendingObservePrepare>>
BY NextSplitsTimeoutViewFrontierSteps,
   StableStepLeavesTimeoutViewFrontier

THEOREM NextPreservesTimeoutViewFrontierShape ==
  (StrongInductiveInvariant
    /\ TimeoutViewFrontierShapeInvariant
    /\ Next)
    => TimeoutViewFrontierShapeInvariant'
PROOF
  <1>1. ASSUME StrongInductiveInvariant,
                TimeoutViewFrontierShapeInvariant,
                Next
         PROVE TimeoutViewFrontierShapeInvariant'
    <2>1. \/ TimeoutViewFrontierMutationStep
           \/ UNCHANGED <<nodeView, highestRank,
                           pendingObservePrepare>>
      BY <1>1, NextEitherMutatesTimeoutViewFrontierOrLeavesIt
    <2>2. CASE TimeoutViewFrontierMutationStep
      BY <1>1, <2>2,
         TimeoutViewFrontierMutationPreservesShape
    <2>3. CASE UNCHANGED <<nodeView, highestRank,
                           pendingObservePrepare>>
      BY <1>1, <2>3,
         UnchangedTimeoutViewFrontierPreservesShape
    <2> QED BY <2>1, <2>2, <2>3
  <1> QED BY <1>1

THEOREM InitAtEstablishesTimeoutViewFrontierShape ==
  \A initialContext:
    InitAt(initialContext) => TimeoutViewFrontierShapeInvariant
BY SMT
   DEF InitAt, TimeoutViewFrontierShapeInvariant,
       HighestNotAheadOfView, PendingObserveViewBound,
       NoRank, Views, ModelConfiguration

THEOREM InitAtEstablishesStrongTimeoutViewFrontierInvariant ==
  \A initialContext:
    InitAt(initialContext) => StrongTimeoutViewFrontierInvariant
BY InitAtEstablishesStrongInductiveInvariant,
   InitAtEstablishesTimeoutViewFrontierShape
   DEF StrongTimeoutViewFrontierInvariant

THEOREM CoreActionPreservesStrongTimeoutViewFrontierInvariant ==
  (StrongTimeoutViewFrontierInvariant /\ [Next]_vars)
    => StrongTimeoutViewFrontierInvariant'
PROOF
  <1>1. ASSUME StrongTimeoutViewFrontierInvariant,
                [Next]_vars
         PROVE StrongTimeoutViewFrontierInvariant'
    <2>1. StrongInductiveInvariant'
      BY <1>1, CoreStrongInductiveActionPreservation
         DEF StrongTimeoutViewFrontierInvariant
    <2>2. TimeoutViewFrontierShapeInvariant'
      <3>1. CASE Next
        BY <1>1, <3>1, NextPreservesTimeoutViewFrontierShape
           DEF StrongTimeoutViewFrontierInvariant,
               StrongInductiveInvariant
      <3>2. CASE UNCHANGED vars
        BY <1>1, <3>2,
           UnchangedTimeoutViewFrontierPreservesShape
           DEF StrongTimeoutViewFrontierInvariant, vars
      <3> QED BY <1>1, <3>1, <3>2
    <2> QED BY <2>1, <2>2
       DEF StrongTimeoutViewFrontierInvariant
  <1> QED BY <1>1

THEOREM CoreSpecAtAlwaysStrongTimeoutViewFrontierInvariant ==
  \A initialContext:
    CoreSpecAt(initialContext)
      => []StrongTimeoutViewFrontierInvariant
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE CoreSpecAt(initialContext)
                 => []StrongTimeoutViewFrontierInvariant
    <2>1. InitAt(initialContext)
             => StrongTimeoutViewFrontierInvariant
      BY InitAtEstablishesStrongTimeoutViewFrontierInvariant
    <2>2. StrongTimeoutViewFrontierInvariant /\ [Next]_vars
             => StrongTimeoutViewFrontierInvariant'
      BY CoreActionPreservesStrongTimeoutViewFrontierInvariant
    <2> QED BY <2>1, <2>2, PTL DEF CoreSpecAt
  <1> QED BY <1>1

=============================================================================
