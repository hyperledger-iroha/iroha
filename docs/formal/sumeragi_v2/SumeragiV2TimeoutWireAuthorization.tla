---- MODULE SumeragiV2TimeoutWireAuthorization ----
EXTENDS SumeragiV2TimeoutSigningInvariant, SumeragiV2TimeoutViewInvariant

(***************************************************************************
Inductive timeout-wire authorization.  A local timeout remains tied to the
frozen voter roster, current context and height, an authenticated PrepareQC
high reference, and a high rank no greater than the timeout view across the
pending WAL, durable intent, signing, and honest transport frontiers.
***************************************************************************)

TimeoutVoteWireAuthorized(vote) ==
  /\ vote \in TimeoutVoteRecordSet
  /\ vote.signer \in Honest
  /\ vote.signer \in CurrentVoters
  /\ vote.context = context
  /\ vote.height = height
  /\ AuthenticatedHighRef(vote.highRank, vote.highSubject)
  /\ vote.highRank <= vote.view

TimeoutRequestWireAuthorized(request) ==
  /\ request.node \in Honest \cap CurrentVoters
  /\ request.vote.signer = request.node
  /\ TimeoutVoteWireAuthorized(request.vote)

PendingTimeoutWireAuthorization ==
  \A request \in pendingTimeout:
    TimeoutRequestWireAuthorized(request)

DurableTimeoutWireAuthorization ==
  \A vote \in timeoutIntents:
    TimeoutVoteWireAuthorized(vote)

TimeoutWireAuthorizationShapeInvariant ==
  /\ PendingTimeoutWireAuthorization
  /\ DurableTimeoutWireAuthorization

StrongTimeoutWireAuthorizationInvariant ==
  /\ StrongTimeoutDurabilityInvariant
  /\ StrongTimeoutViewFrontierInvariant
  /\ TimeoutWireAuthorizationShapeInvariant

CoreAuthorizationFrame ==
  /\ context' = context
  /\ height' = height
  /\ prepareQCs \subseteq prepareQCs'

TimeoutWireMutationStep ==
  \/ \E node \in ValidatorIds: BeginTimeout(node)
  \/ \E request \in pendingTimeout: PersistTimeout(request)
  \/ \E node \in ValidatorIds: Crash(node)

TimeoutWireTimeoutStableStep ==
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

TimeoutWireFrontierStableStep ==
  \/ \E node \in ValidatorIds, qc \in ReceivedQcValues:
       BeginObservePrepare(node, qc)
  \/ \E request \in pendingObservePrepare:
       PersistObservePrepare(request)
  \/ \E request \in pendingLockCommit:
       PersistLockCommit(request)
  \/ \E request \in pendingInstallTC:
       PersistInstallTC(request)

TimeoutWireStableStep ==
  \/ TimeoutViewFrontierProposalStableStep
  \/ TimeoutViewFrontierVoteStableStep
  \/ TimeoutViewFrontierRecoveryStableStep
  \/ TimeoutWireTimeoutStableStep
  \/ TimeoutWireFrontierStableStep

THEOREM AuthenticatedTimeoutHighRefSurvivesPrepareQcGrowth ==
  \A highRank, highSubject:
    (context' = context
      /\ prepareQCs \subseteq prepareQCs'
      /\ AuthenticatedHighRef(highRank, highSubject))
      => AuthenticatedHighRef(highRank, highSubject)'
BY SMT DEF AuthenticatedHighRef, HighRefValid

THEOREM CoreAuthorizationFramePreservesVoteAuthorization ==
  \A vote:
    (CoreAuthorizationFrame /\ TimeoutVoteWireAuthorized(vote))
      => TimeoutVoteWireAuthorized(vote)'
PROOF
  <1>1. ASSUME NEW vote,
                CoreAuthorizationFrame,
                TimeoutVoteWireAuthorized(vote)
         PROVE TimeoutVoteWireAuthorized(vote)'
    <2>1. CurrentVoters' = CurrentVoters
      BY <1>1 DEF CoreAuthorizationFrame, CurrentVoters, CurrentEpoch
    <2>2. AuthenticatedHighRef(vote.highRank, vote.highSubject)'
      BY <1>1, AuthenticatedTimeoutHighRefSurvivesPrepareQcGrowth
         DEF CoreAuthorizationFrame, TimeoutVoteWireAuthorized
    <2> QED BY <1>1, <2>1, <2>2
       DEF CoreAuthorizationFrame, TimeoutVoteWireAuthorized
  <1> QED BY <1>1

THEOREM CoreAuthorizationFramePreservesRequestAuthorization ==
  \A request:
    (CoreAuthorizationFrame /\ TimeoutRequestWireAuthorized(request))
      => TimeoutRequestWireAuthorized(request)'
PROOF
  <1>1. ASSUME NEW request,
                CoreAuthorizationFrame,
                TimeoutRequestWireAuthorized(request)
         PROVE TimeoutRequestWireAuthorized(request)'
    <2>1. CurrentVoters' = CurrentVoters
      BY <1>1 DEF CoreAuthorizationFrame, CurrentVoters, CurrentEpoch
    <2>2. TimeoutVoteWireAuthorized(request.vote)'
      BY <1>1, CoreAuthorizationFramePreservesVoteAuthorization
         DEF TimeoutRequestWireAuthorized
    <2> QED BY <1>1, <2>1, <2>2
       DEF TimeoutRequestWireAuthorized
  <1> QED BY <1>1

THEOREM TimeoutWireFramePreservesAuthorizationShape ==
  (TimeoutWireAuthorizationShapeInvariant
    /\ CoreAuthorizationFrame
    /\ pendingTimeout' = pendingTimeout
    /\ timeoutIntents' = timeoutIntents)
    => TimeoutWireAuthorizationShapeInvariant'
BY CoreAuthorizationFramePreservesVoteAuthorization,
   CoreAuthorizationFramePreservesRequestAuthorization
   DEF TimeoutWireAuthorizationShapeInvariant,
       PendingTimeoutWireAuthorization,
       DurableTimeoutWireAuthorization

THEOREM FrontierMutationSuppliesCoreAuthorizationFrame ==
  TimeoutViewFrontierMutationStep => CoreAuthorizationFrame
BY IsaT(60)
   DEF TimeoutViewFrontierMutationStep, CoreAuthorizationFrame,
       BeginObservePrepare, PersistObservePrepare,
       PersistLockCommit, PersistInstallTC, Crash

THEOREM ProposalStableSuppliesCoreAuthorizationFrame ==
  TimeoutViewFrontierProposalStableStep => CoreAuthorizationFrame
BY IsaT(60)
   DEF TimeoutViewFrontierProposalStableStep, CoreAuthorizationFrame,
       SetGST, AssembleLocalBody, BeginLocalProposal,
       PersistProposal, CompleteProposalSignature,
       ByzantineBroadcastProposal, DeliverProposal,
       FetchBody, RebindRetainedBody, StoreBody,
       ValidateBody, RejectBody, ValidateDecidedBody

THEOREM VoteStableSuppliesCoreAuthorizationFrame ==
  TimeoutViewFrontierVoteStableStep => CoreAuthorizationFrame
BY IsaT(60)
   DEF TimeoutViewFrontierVoteStableStep, CoreAuthorizationFrame,
       BeginPrepare, PersistPrepare, CompleteVoteSignature,
       ByzantineBroadcastVote, DeliverVote, FormPrepareQC, DeliverQC,
       BeginLockCommit, FormCommitQC, BeginDecision, PersistDecision

THEOREM TimeoutStableSuppliesCoreAuthorizationFrame ==
  TimeoutViewFrontierTimeoutStableStep => CoreAuthorizationFrame
BY IsaT(60)
   DEF TimeoutViewFrontierTimeoutStableStep, CoreAuthorizationFrame,
       BeginTimeout, PersistTimeout, CompleteTimeoutSignature,
       ByzantineBroadcastTimeout, DeliverTimeout, FormTC, DeliverTC,
       BeginInstallTC

THEOREM RecoveryStableSuppliesCoreAuthorizationFrame ==
  TimeoutViewFrontierRecoveryStableStep => CoreAuthorizationFrame
BY IsaT(60)
   DEF TimeoutViewFrontierRecoveryStableStep, CoreAuthorizationFrame,
       FetchCertifiedBody, ApplyDecision, Restart,
       ResumeProposal, ResumeVote, ResumeTimeout, DropProposal

THEOREM FrontierStableSuppliesCoreAuthorizationFrame ==
  TimeoutViewFrontierStableStep => CoreAuthorizationFrame
BY FrontierMutationSuppliesCoreAuthorizationFrame,
   ProposalStableSuppliesCoreAuthorizationFrame,
   VoteStableSuppliesCoreAuthorizationFrame,
   TimeoutStableSuppliesCoreAuthorizationFrame,
   RecoveryStableSuppliesCoreAuthorizationFrame
   DEF TimeoutViewFrontierStableStep

THEOREM NextSuppliesCoreAuthorizationFrame ==
  Next => CoreAuthorizationFrame
BY NextSplitsTimeoutViewFrontierSteps,
   FrontierMutationSuppliesCoreAuthorizationFrame,
   FrontierStableSuppliesCoreAuthorizationFrame

THEOREM BeginTimeoutCreatesWireAuthorizedRequest ==
  \A node \in ValidatorIds:
    (/\ StrongTimeoutDurabilityInvariant
     /\ StrongTimeoutViewFrontierInvariant
     /\ BeginTimeout(node))
      => TimeoutRequestWireAuthorized(TimeoutRequestFor(node))
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                StrongTimeoutDurabilityInvariant,
                StrongTimeoutViewFrontierInvariant,
                BeginTimeout(node)
         PROVE TimeoutRequestWireAuthorized(TimeoutRequestFor(node))
    <2>1. /\ TypeInvariant
           /\ HighestAndLockAreCertified
           /\ node \in Honest \cap CurrentVoters
           /\ TimeoutRequestFor(node) \in TimeoutWalSet
      BY <1>1
         DEF StrongTimeoutViewFrontierInvariant,
             StrongTimeoutDurabilityInvariant,
             StrongInductiveInvariant, Safety,
             ReducerProvenanceInvariant, BeginTimeout
    <2>2. AuthenticatedHighRef(
             LocalTimeoutVoteFor(node).highRank,
             LocalTimeoutVoteFor(node).highSubject)
      BY <1>1, <2>1, LocalTimeoutHighRefIsValid
         DEF AuthenticatedHighRef
    <2>3. highestRank[node] <= nodeView[node]
      BY <1>1
         DEF StrongTimeoutViewFrontierInvariant,
             TimeoutViewFrontierShapeInvariant,
             HighestNotAheadOfView
    <2>4. /\ TimeoutRequestFor(node).node = node
           /\ TimeoutRequestFor(node).vote = LocalTimeoutVoteFor(node)
           /\ LocalTimeoutVoteFor(node) \in TimeoutVoteRecordSet
           /\ LocalTimeoutVoteFor(node).signer = node
           /\ LocalTimeoutVoteFor(node).context = context
           /\ LocalTimeoutVoteFor(node).height = height
           /\ LocalTimeoutVoteFor(node).highRank = highestRank[node]
           /\ LocalTimeoutVoteFor(node).view = nodeView[node]
      BY <1>1, <2>1, Isa
         DEF TimeoutRequestFor, TimeoutWal, TimeoutWalSet,
             LocalTimeoutVoteFor, TimeoutVote, TypeInvariant
    <2> QED BY <2>1, <2>2, <2>3, <2>4
       DEF TimeoutRequestWireAuthorized,
           TimeoutVoteWireAuthorized
  <1> QED BY <1>1

THEOREM BeginTimeoutPreservesWireAuthorizationShape ==
  \A node \in ValidatorIds:
    (StrongTimeoutWireAuthorizationInvariant /\ BeginTimeout(node))
      => TimeoutWireAuthorizationShapeInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                StrongTimeoutWireAuthorizationInvariant,
                BeginTimeout(node)
         PROVE TimeoutWireAuthorizationShapeInvariant'
    <2>1. CoreAuthorizationFrame
      BY <1>1 DEF CoreAuthorizationFrame, BeginTimeout
    <2>2. TimeoutRequestWireAuthorized(TimeoutRequestFor(node))
      BY <1>1, BeginTimeoutCreatesWireAuthorizedRequest
         DEF StrongTimeoutWireAuthorizationInvariant
    <2>3. /\ pendingTimeout' =
                  pendingTimeout \cup {TimeoutRequestFor(node)}
           /\ timeoutIntents' = timeoutIntents
      BY <1>1 DEF BeginTimeout
    <2>4. PendingTimeoutWireAuthorization'
      <3>1. ASSUME NEW request \in pendingTimeout'
             PROVE TimeoutRequestWireAuthorized(request)'
        <4>1. request \in pendingTimeout
               \/ request = TimeoutRequestFor(node)
          BY <2>3, <3>1
        <4>2. CASE request \in pendingTimeout
          <5>1. TimeoutRequestWireAuthorized(request)
            BY <1>1, <4>2
               DEF StrongTimeoutWireAuthorizationInvariant,
                   TimeoutWireAuthorizationShapeInvariant,
                   PendingTimeoutWireAuthorization
          <5> QED BY <2>1, <5>1,
             CoreAuthorizationFramePreservesRequestAuthorization
        <4>3. CASE request = TimeoutRequestFor(node)
          <5>1. TimeoutRequestWireAuthorized(request)
            BY <2>2, <4>3
          <5> QED BY <2>1, <5>1,
             CoreAuthorizationFramePreservesRequestAuthorization
        <4> QED BY <4>1, <4>2, <4>3
      <3> QED BY <3>1 DEF PendingTimeoutWireAuthorization
    <2>5. DurableTimeoutWireAuthorization'
      BY <1>1, <2>1, <2>3,
         CoreAuthorizationFramePreservesVoteAuthorization
         DEF StrongTimeoutWireAuthorizationInvariant,
             TimeoutWireAuthorizationShapeInvariant,
             DurableTimeoutWireAuthorization
    <2> QED BY <2>4, <2>5
       DEF TimeoutWireAuthorizationShapeInvariant
  <1> QED BY <1>1

THEOREM PersistTimeoutPreservesWireAuthorizationShape ==
  \A request \in pendingTimeout:
    (TimeoutWireAuthorizationShapeInvariant /\ PersistTimeout(request))
      => TimeoutWireAuthorizationShapeInvariant'
PROOF
  <1>1. ASSUME NEW request \in pendingTimeout,
                TimeoutWireAuthorizationShapeInvariant,
                PersistTimeout(request)
         PROVE TimeoutWireAuthorizationShapeInvariant'
    <2>1. CoreAuthorizationFrame
      BY <1>1 DEF CoreAuthorizationFrame, PersistTimeout
    <2>2. TimeoutRequestWireAuthorized(request)
      BY <1>1
         DEF TimeoutWireAuthorizationShapeInvariant,
             PendingTimeoutWireAuthorization
    <2>3. TimeoutVoteWireAuthorized(request.vote)'
      BY <2>1, <2>2,
         CoreAuthorizationFramePreservesVoteAuthorization
         DEF TimeoutRequestWireAuthorized
    <2>4. /\ pendingTimeout' = pendingTimeout \ {request}
           /\ timeoutIntents' = timeoutIntents \cup {request.vote}
      BY <1>1 DEF PersistTimeout
    <2>5. PendingTimeoutWireAuthorization'
      BY <1>1, <2>1, <2>4,
         CoreAuthorizationFramePreservesRequestAuthorization, Isa
         DEF TimeoutWireAuthorizationShapeInvariant,
             PendingTimeoutWireAuthorization
    <2>6. DurableTimeoutWireAuthorization'
      BY <1>1, <2>1, <2>3, <2>4,
         CoreAuthorizationFramePreservesVoteAuthorization, Isa
         DEF TimeoutWireAuthorizationShapeInvariant,
             DurableTimeoutWireAuthorization
    <2> QED BY <2>5, <2>6
       DEF TimeoutWireAuthorizationShapeInvariant
  <1> QED BY <1>1

THEOREM CrashPreservesWireAuthorizationShape ==
  \A node \in ValidatorIds:
    (TimeoutWireAuthorizationShapeInvariant /\ Crash(node))
      => TimeoutWireAuthorizationShapeInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                TimeoutWireAuthorizationShapeInvariant,
                Crash(node)
         PROVE TimeoutWireAuthorizationShapeInvariant'
    <2>1. CoreAuthorizationFrame
      BY <1>1 DEF CoreAuthorizationFrame, Crash
    <2>2. /\ pendingTimeout' \subseteq pendingTimeout
           /\ timeoutIntents' = timeoutIntents
      BY <1>1, Isa DEF Crash
    <2> QED BY <1>1, <2>1, <2>2,
         CoreAuthorizationFramePreservesVoteAuthorization,
         CoreAuthorizationFramePreservesRequestAuthorization, Isa
       DEF TimeoutWireAuthorizationShapeInvariant,
           PendingTimeoutWireAuthorization,
           DurableTimeoutWireAuthorization
  <1> QED BY <1>1

THEOREM TimeoutWireMutationPreservesAuthorizationShape ==
  (StrongTimeoutWireAuthorizationInvariant /\ TimeoutWireMutationStep)
    => TimeoutWireAuthorizationShapeInvariant'
PROOF
  <1>1. ASSUME StrongTimeoutWireAuthorizationInvariant,
                TimeoutWireMutationStep
         PROVE TimeoutWireAuthorizationShapeInvariant'
    <2>1. CASE \E node \in ValidatorIds: BeginTimeout(node)
      BY <1>1, <2>1, BeginTimeoutPreservesWireAuthorizationShape
    <2>2. CASE \E request \in pendingTimeout: PersistTimeout(request)
      BY <1>1, <2>2, PersistTimeoutPreservesWireAuthorizationShape
         DEF StrongTimeoutWireAuthorizationInvariant
    <2>3. CASE \E node \in ValidatorIds: Crash(node)
      BY <1>1, <2>3, CrashPreservesWireAuthorizationShape
         DEF StrongTimeoutWireAuthorizationInvariant
    <2> QED BY <1>1, <2>1, <2>2, <2>3
         DEF TimeoutWireMutationStep
  <1> QED BY <1>1

THEOREM ProposalStableStepLeavesTimeoutWireSets ==
  TimeoutViewFrontierProposalStableStep
    => UNCHANGED <<pendingTimeout, timeoutIntents>>
BY IsaT(60)
   DEF TimeoutViewFrontierProposalStableStep,
       SetGST, AssembleLocalBody, BeginLocalProposal,
       PersistProposal, CompleteProposalSignature,
       ByzantineBroadcastProposal, DeliverProposal,
       FetchBody, RebindRetainedBody, StoreBody,
       ValidateBody, RejectBody, ValidateDecidedBody

THEOREM VoteStableStepLeavesTimeoutWireSets ==
  TimeoutViewFrontierVoteStableStep
    => UNCHANGED <<pendingTimeout, timeoutIntents>>
BY IsaT(60)
   DEF TimeoutViewFrontierVoteStableStep,
       BeginPrepare, PersistPrepare, CompleteVoteSignature,
       ByzantineBroadcastVote, DeliverVote, FormPrepareQC, DeliverQC,
       BeginLockCommit, FormCommitQC, BeginDecision, PersistDecision

THEOREM RecoveryStableStepLeavesTimeoutWireSets ==
  TimeoutViewFrontierRecoveryStableStep
    => UNCHANGED <<pendingTimeout, timeoutIntents>>
BY IsaT(60)
   DEF TimeoutViewFrontierRecoveryStableStep,
       FetchCertifiedBody, ApplyDecision, Restart,
       ResumeProposal, ResumeVote, ResumeTimeout, DropProposal

THEOREM TimeoutStableStepLeavesTimeoutWireSets ==
  TimeoutWireTimeoutStableStep
    => UNCHANGED <<pendingTimeout, timeoutIntents>>
BY IsaT(60)
   DEF TimeoutWireTimeoutStableStep,
       CompleteTimeoutSignature, ByzantineBroadcastTimeout,
       DeliverTimeout, FormTC, DeliverTC, BeginInstallTC

THEOREM FrontierStableStepLeavesTimeoutWireSets ==
  TimeoutWireFrontierStableStep
    => UNCHANGED <<pendingTimeout, timeoutIntents>>
BY IsaT(60)
   DEF TimeoutWireFrontierStableStep,
       BeginObservePrepare, PersistObservePrepare,
       PersistLockCommit, PersistInstallTC

THEOREM TimeoutWireStableStepLeavesTimeoutWireSets ==
  TimeoutWireStableStep
    => UNCHANGED <<pendingTimeout, timeoutIntents>>
BY ProposalStableStepLeavesTimeoutWireSets,
   VoteStableStepLeavesTimeoutWireSets,
   RecoveryStableStepLeavesTimeoutWireSets,
   TimeoutStableStepLeavesTimeoutWireSets,
   FrontierStableStepLeavesTimeoutWireSets
   DEF TimeoutWireStableStep

THEOREM NextSplitsTimeoutWireSteps ==
  Next => TimeoutWireMutationStep \/ TimeoutWireStableStep
BY Isa
   DEF Next, TimeoutWireMutationStep, TimeoutWireStableStep,
       TimeoutViewFrontierProposalStableStep,
       TimeoutViewFrontierVoteStableStep,
       TimeoutViewFrontierRecoveryStableStep,
       TimeoutWireTimeoutStableStep,
       TimeoutWireFrontierStableStep

THEOREM NextPreservesTimeoutWireAuthorizationShape ==
  (StrongTimeoutWireAuthorizationInvariant /\ Next)
    => TimeoutWireAuthorizationShapeInvariant'
PROOF
  <1>1. ASSUME StrongTimeoutWireAuthorizationInvariant,
                Next
         PROVE TimeoutWireAuthorizationShapeInvariant'
    <2>1. CoreAuthorizationFrame
      BY <1>1, NextSuppliesCoreAuthorizationFrame
    <2>2. TimeoutWireMutationStep \/ TimeoutWireStableStep
      BY <1>1, NextSplitsTimeoutWireSteps
    <2>3. CASE TimeoutWireMutationStep
      BY <1>1, <2>3,
         TimeoutWireMutationPreservesAuthorizationShape
    <2>4. CASE TimeoutWireStableStep
      <3>1. UNCHANGED <<pendingTimeout, timeoutIntents>>
        BY <2>4, TimeoutWireStableStepLeavesTimeoutWireSets
      <3> QED BY <1>1, <2>1, <3>1,
           TimeoutWireFramePreservesAuthorizationShape
           DEF StrongTimeoutWireAuthorizationInvariant
    <2> QED BY <2>2, <2>3, <2>4
  <1> QED BY <1>1

THEOREM InitAtEstablishesTimeoutWireAuthorizationShape ==
  \A initialContext:
    InitAt(initialContext) => TimeoutWireAuthorizationShapeInvariant
BY DEF InitAt, TimeoutWireAuthorizationShapeInvariant,
       PendingTimeoutWireAuthorization,
       DurableTimeoutWireAuthorization

THEOREM InitAtEstablishesStrongTimeoutWireAuthorizationInvariant ==
  \A initialContext:
    InitAt(initialContext) => StrongTimeoutWireAuthorizationInvariant
BY InitAtEstablishesStrongTimeoutDurabilityInvariant,
   InitAtEstablishesStrongTimeoutViewFrontierInvariant,
   InitAtEstablishesTimeoutWireAuthorizationShape
   DEF StrongTimeoutWireAuthorizationInvariant

THEOREM CoreActionPreservesStrongTimeoutWireAuthorizationInvariant ==
  (StrongTimeoutWireAuthorizationInvariant /\ [Next]_vars)
    => StrongTimeoutWireAuthorizationInvariant'
PROOF
  <1>1. ASSUME StrongTimeoutWireAuthorizationInvariant,
                [Next]_vars
         PROVE StrongTimeoutWireAuthorizationInvariant'
    <2>1. StrongTimeoutDurabilityInvariant'
      BY <1>1, CoreActionPreservesStrongTimeoutDurabilityInvariant
         DEF StrongTimeoutWireAuthorizationInvariant
    <2>2. StrongTimeoutViewFrontierInvariant'
      BY <1>1, CoreActionPreservesStrongTimeoutViewFrontierInvariant
         DEF StrongTimeoutWireAuthorizationInvariant
    <2>3. TimeoutWireAuthorizationShapeInvariant'
      <3>1. CASE Next
        BY <1>1, <3>1,
           NextPreservesTimeoutWireAuthorizationShape
      <3>2. CASE UNCHANGED vars
        <4>1. /\ CoreAuthorizationFrame
               /\ pendingTimeout' = pendingTimeout
               /\ timeoutIntents' = timeoutIntents
          BY <3>2 DEF CoreAuthorizationFrame, vars
        <4> QED BY <1>1, <4>1,
             TimeoutWireFramePreservesAuthorizationShape
             DEF StrongTimeoutWireAuthorizationInvariant
      <3> QED BY <1>1, <3>1, <3>2
    <2> QED BY <2>1, <2>2, <2>3
       DEF StrongTimeoutWireAuthorizationInvariant
  <1> QED BY <1>1

THEOREM CoreSpecAtAlwaysStrongTimeoutWireAuthorizationInvariant ==
  \A initialContext:
    CoreSpecAt(initialContext)
      => []StrongTimeoutWireAuthorizationInvariant
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE CoreSpecAt(initialContext)
                 => []StrongTimeoutWireAuthorizationInvariant
    <2>1. InitAt(initialContext)
             => StrongTimeoutWireAuthorizationInvariant
      BY InitAtEstablishesStrongTimeoutWireAuthorizationInvariant
    <2>2. StrongTimeoutWireAuthorizationInvariant /\ [Next]_vars
             => StrongTimeoutWireAuthorizationInvariant'
      BY CoreActionPreservesStrongTimeoutWireAuthorizationInvariant
    <2> QED BY <2>1, <2>2, PTL DEF CoreSpecAt
  <1> QED BY <1>1

THEOREM StrongWireInvariantAuthorizesPendingTimeoutSignature ==
  \A request \in signTimeouts:
    StrongTimeoutWireAuthorizationInvariant
      => TimeoutRequestWireAuthorized(request)
PROOF
  <1>1. ASSUME NEW request \in signTimeouts,
                StrongTimeoutWireAuthorizationInvariant
         PROVE TimeoutRequestWireAuthorized(request)
    <2>1. /\ request.vote \in timeoutIntents
           /\ request.vote.signer = request.node
      BY <1>1, PendingTimeoutSignatureIsAuthorized
         DEF StrongTimeoutWireAuthorizationInvariant,
             StrongTimeoutViewFrontierInvariant,
             StrongTimeoutDurabilityInvariant
    <2>2. TimeoutVoteWireAuthorized(request.vote)
      BY <1>1, <2>1
         DEF StrongTimeoutWireAuthorizationInvariant,
             TimeoutWireAuthorizationShapeInvariant,
             DurableTimeoutWireAuthorization
    <2> QED BY <2>1, <2>2
       DEF TimeoutRequestWireAuthorized,
           TimeoutVoteWireAuthorized
  <1> QED BY <1>1

THEOREM StrongWireInvariantAuthorizesHonestTimeoutEnvelope ==
  \A envelope \in timeoutNetwork:
    (StrongTimeoutWireAuthorizationInvariant
      /\ envelope.vote.signer \in Honest)
      => TimeoutVoteWireAuthorized(envelope.vote)
PROOF
  <1>1. ASSUME NEW envelope \in timeoutNetwork,
                StrongTimeoutWireAuthorizationInvariant,
                envelope.vote.signer \in Honest
         PROVE TimeoutVoteWireAuthorized(envelope.vote)
    <2>1. envelope.vote \in timeoutIntents
      BY <1>1
         DEF StrongTimeoutWireAuthorizationInvariant,
             StrongTimeoutViewFrontierInvariant,
             StrongTimeoutDurabilityInvariant,
             StrongInductiveInvariant,
             ReducerProvenanceInvariant,
             HonestTimeoutTransportBacked
    <2> QED BY <1>1, <2>1
       DEF StrongTimeoutWireAuthorizationInvariant,
           TimeoutWireAuthorizationShapeInvariant,
           DurableTimeoutWireAuthorization
  <1> QED BY <1>1

=============================================================================

