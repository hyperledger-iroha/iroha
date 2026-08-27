---- MODULE SumeragiV2AsyncRecoveryProgressWitnessProofs ----
EXTENDS SumeragiV2AsyncSchedulerCompositionProofs

(***************************************************************************
The current locked Commit intent has one durable source owner: either its
signature request is pending, or the exact signed batch is retained by the
adapter.  The two supporting invariants capture the monotone lock frontier
and the provenance/order of responsive retained Commit controls.
***************************************************************************)

ResponsiveCommitIntentLockBound ==
  \A vote \in commitIntents:
    (vote.signer \in AsyncCurrentResponsiveVoters
      /\ vote.context = context)
      => /\ vote.view <= lockRank[vote.signer]
         /\ (vote.view = lockRank[vote.signer]
               => vote.subject = lockSubject[vote.signer])

ResponsiveRetainedCommitControlSound ==
  \A item \in asyncRetainedControl:
    (item.kind = "CommitVote"
      /\ item.source \in AsyncCurrentResponsiveVoters)
      => /\ item.envelope.vote.signer = item.source
         /\ item.envelope.vote.context = context
         /\ item.envelope.vote \in commitIntents
         /\ item.envelope.vote.view <= lockRank[item.source]

CommitRecoveryAuthority(node) ==
  /\ asyncRecoveryPhase
       \in {"RestartRequired", "ReplayRequired", "Replaying"}
  /\ asyncRecoveryNode = node
  /\ generation[node] = asyncRecoveryGeneration

AsyncCommitIntentProgressWitness(node, vote) ==
  \/ CommitIntentProgressWitness(node, vote)
  \/ CommitRecoveryAuthority(node)

AsyncDurableCommitProgressWitness ==
  \A node \in AsyncCurrentResponsiveVoters, vote \in commitIntents:
    ActiveLockedCommitIntent(node, vote)
      => AsyncCommitIntentProgressWitness(node, vote)

THEOREM PreGstResponsiveCrashReplacesCommitSignatureWithRecoveryAuthority ==
  \A node \in ValidatorIds, vote \in VoteRecordSet:
    /\ VoteSign(node, vote) \in signVotes
    /\ PreGstResponsiveCrash(node)
    => /\ VoteSign(node, vote) \notin signVotes'
       /\ CommitRecoveryAuthority(node)'
       /\ AsyncCommitIntentProgressWitness(node, vote)'
BY Isa
   DEF PreGstResponsiveCrash, Crash, VoteSign, CommitRecoveryAuthority,
       AsyncCommitIntentProgressWitness, CommitIntentProgressWitness,
       RetainedCommitIntent, NodeHasDecision

ActiveCommitIntentSourceOwnership ==
  /\ \A request \in signVotes:
       /\ request.node \in AsyncCurrentResponsiveVoters
       /\ request.vote.phase = "Commit"
       => /\ request.vote.signer = request.node
          /\ request.vote.context = context
          /\ request.vote \in commitIntents
          /\ request.vote.view <= lockRank[request.node]
          /\ (request.vote.view = lockRank[request.node]
                => request.vote.subject = lockSubject[request.node])
  /\ \A node \in AsyncCurrentResponsiveVoters, vote \in commitIntents:
       ActiveLockedCommitIntent(node, vote)
         => AsyncCommitIntentProgressWitness(node, vote)

CommitSourceRetentionInvariant ==
  /\ ResponsiveCommitIntentLockBound
  /\ ResponsiveRetainedCommitControlSound
  /\ ActiveCommitIntentSourceOwnership

CommitSourceAuxiliaryInvariant ==
  /\ ResponsiveRetainedCommitControlSound
  /\ ActiveCommitIntentSourceOwnership

ResponsiveCommitSignRequests ==
  {request \in signVotes:
     /\ request.node \in AsyncCurrentResponsiveVoters
     /\ request.vote.phase = "Commit"}

ResponsiveRetainedCommitItems ==
  {item \in asyncRetainedControl:
     /\ item.source \in AsyncCurrentResponsiveVoters
     /\ item.kind = "CommitVote"}

CommitCoreSourceProjection ==
  <<context, commitIntents, receivedVotes, commitQCs, decisions,
    nodeView, generation, lockRank, lockSubject,
    ResponsiveCommitSignRequests, ResponsiveRetainedCommitItems>>

CommitSourceProjection ==
  <<CommitCoreSourceProjection, AsyncRecoveryControlVars>>

PersistLockCommitSourceTransition(request) ==
  /\ PersistLockCommit(request)
  /\ UNCHANGED ResponsiveRetainedCommitItems

PersistInstallCommitSourceTransition(request) ==
  /\ PersistInstallTC(request)
  /\ ActiveLockedCommitSignRequestsAfterInstall(
       request.node, request.tc) \subseteq signVotes'
  /\ UNCHANGED ResponsiveRetainedCommitItems

ResponsiveCrashCommitSourceTransition(node) ==
  PreGstResponsiveCrash(node)

ResponsiveRestartCommitSourceTransition ==
  PreGstResponsiveRestart

ResponsiveReplayCommitSourceTransition ==
  PreGstResponsiveReplay

ResponsiveReplayContinuationCommitSourceTransition ==
  \/ DriveResponsiveReplayHead
  \/ FinishResponsiveReplay
  \/ RearmResponsiveRecovery

THEOREM WeakStrictOrderTrans ==
  \A low, middle, high:
    low <= middle /\ middle < high => low < high
BY SMT

THEOREM WeakOrderTrans ==
  \A low, middle, high:
    low <= middle /\ middle <= high => low <= high
BY SMT

THEOREM SqueezedOrderEquality ==
  \A low, middle, high:
    /\ low <= middle
    /\ middle <= high
    /\ low = high
    => /\ low = middle
       /\ middle = high
BY SMT

THEOREM ProgressFunctionalUpdateAtKey ==
  \A mapping, key, value:
    key \in DOMAIN mapping
      => [mapping EXCEPT ![key] = value][key] = value
BY Isa

THEOREM AsyncInitEstablishesCommitSourceRetention ==
  \A initialContext:
    AsyncInitAt(initialContext) => CommitSourceRetentionInvariant
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncInitAt(initialContext)
         PROVE CommitSourceRetentionInvariant
    <2>1. /\ context = initialContext
           /\ asyncRetainedControl = {}
           /\ ModelConfiguration
           /\ FrozenContextAdmissible(initialContext)
           /\ initialContext.height \in Nat
           /\ (initialContext.height = 0 => commitIntents = {})
           /\ (initialContext.height > 0
                 => /\ commitIntents =
                          BootstrapParentCommitIntents(initialContext)
                    /\ BootstrapParentContext(initialContext)
                         # initialContext)
      <3>1. /\ ModelConfiguration
             /\ FrozenContextAdmissible(initialContext)
             /\ context = initialContext
             /\ asyncRetainedControl = {}
             /\ (initialContext.height = 0 => commitIntents = {})
             /\ (initialContext.height > 0
                   => commitIntents =
                        BootstrapParentCommitIntents(initialContext))
        BY <1>1, Isa
           DEF AsyncInitAt, AsyncBaseInitAt, InitAt, AsyncTransportInit
      <3>2. initialContext.height \in Nat
        BY <3>1, FrozenContextFieldsTyped DEF Heights
      <3>3. initialContext.height > 0
               => BootstrapParentContext(initialContext) # initialContext
        BY <3>1, BootstrapParentContextPrecedes
      <3> QED BY <3>1, <3>2, <3>3
    <2>2. ResponsiveCommitIntentLockBound
      <3>1. ASSUME NEW vote \in commitIntents,
                    /\ vote.signer \in AsyncCurrentResponsiveVoters
                       /\ vote.context = context
             PROVE /\ vote.view <= lockRank[vote.signer]
                   /\ (vote.view = lockRank[vote.signer]
                         => vote.subject = lockSubject[vote.signer])
        <4>1. CASE initialContext.height = 0
          BY <2>1, <3>1, <4>1
        <4>2. CASE initialContext.height > 0
          <5>1. vote.context.height =
                   BootstrapParentContext(initialContext).height
            BY <2>1, <3>1, <4>2,
               BootstrapParentIntentContextHeights
          <5>2. BootstrapParentContext(initialContext).height #
                   initialContext.height
            BY <2>1, <4>2, BootstrapParentContextPrecedes
          <5>3. vote.context.height = initialContext.height
            BY <2>1, <3>1
          <5> QED BY <5>1, <5>2, <5>3
        <4> QED BY <2>1, <4>1, <4>2, SMT
      <3> QED BY <3>1 DEF ResponsiveCommitIntentLockBound
    <2>3. ResponsiveRetainedCommitControlSound
      BY <2>1 DEF ResponsiveRetainedCommitControlSound
    <2>4. ActiveCommitIntentSourceOwnership
      <3>1. \A request \in signVotes:
               /\ request.node \in AsyncCurrentResponsiveVoters
               /\ request.vote.phase = "Commit"
               => /\ request.vote.signer = request.node
                  /\ request.vote.context = context
                  /\ request.vote \in commitIntents
                  /\ request.vote.view <= lockRank[request.node]
                  /\ (request.vote.view = lockRank[request.node]
                        => request.vote.subject = lockSubject[request.node])
        BY <1>1 DEF AsyncInitAt, AsyncBaseInitAt, InitAt
      <3>2. ASSUME NEW node \in AsyncCurrentResponsiveVoters,
                    NEW vote \in commitIntents,
                    ActiveLockedCommitIntent(node, vote)
             PROVE \/ VoteSign(node, vote) \in signVotes
                   \/ RetainedCommitIntent(node, vote)
        <4>1. CASE initialContext.height = 0
          BY <2>1, <3>2, <4>1
        <4>2. CASE initialContext.height > 0
          <5>1. vote.context.height =
                   BootstrapParentContext(initialContext).height
            BY <2>1, <3>2, <4>2,
               BootstrapParentIntentContextHeights
          <5>2. BootstrapParentContext(initialContext).height #
                   initialContext.height
            BY <2>1, <4>2, BootstrapParentContextPrecedes
          <5>3. vote.context.height = initialContext.height
            BY <2>1, <3>2 DEF ActiveLockedCommitIntent
          <5> QED BY <5>1, <5>2, <5>3
        <4> QED BY <2>1, <4>1, <4>2, SMT
      <3> QED BY <3>1, <3>2
           DEF ActiveCommitIntentSourceOwnership,
               AsyncCommitIntentProgressWitness,
               CommitIntentProgressWitness
    <2> QED BY <2>2, <2>3, <2>4
         DEF CommitSourceRetentionInvariant
  <1> QED BY <1>1

THEOREM LockStableNextLeavesCommitLockFrame ==
  LockStableNext
    => UNCHANGED <<context, commitIntents, lockRank, lockSubject>>
BY IsaM("blast")
   DEF LockStableNext,
       SetGST, AssembleLocalBody, BeginLocalProposal, PersistProposal,
       CompleteProposalSignature, ByzantineBroadcastProposal,
       DeliverProposal, FetchBody, RebindRetainedBody, StoreBody,
       ValidateBody, ValidateDecidedBody, ValidateLockedBody, RejectBody,
       BeginPrepare, PersistPrepare,
       CompleteVoteSignature, ByzantineBroadcastVote, DeliverVote,
       FormPrepareQC, DeliverQC, BeginObservePrepare,
       PersistObservePrepare, BeginLockCommit, FormCommitQC,
       BeginDecision, PersistDecision, BeginTimeout, PersistTimeout,
       CompleteTimeoutSignature, ByzantineBroadcastTimeout,
       DeliverTimeout, DeliverTC, BeginInstallTC,
       FetchCertifiedBody, ApplyDecision, Crash, Restart, ResumeProposal,
       ResumeVote, ResumeTimeout, DropProposal

THEOREM PersistLockPreservesResponsiveCommitIntentLockBound ==
  \A request \in pendingLockCommit:
    /\ StrongInductiveInvariant
    /\ ResponsiveCommitIntentLockBound
    /\ PersistLockCommit(request)
    => ResponsiveCommitIntentLockBound'
PROOF
  <1>1. ASSUME NEW request \in pendingLockCommit,
                StrongInductiveInvariant,
                ResponsiveCommitIntentLockBound,
                PersistLockCommit(request)
         PROVE ResponsiveCommitIntentLockBound'
    <2>1. PendingVoteWritesAuthorized
      BY <1>1
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant
    <2>1a. /\ Honest \subseteq ValidatorIds
            /\ DOMAIN lockRank = ValidatorIds
            /\ DOMAIN lockSubject = ValidatorIds
      BY <1>1, Isa
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             ModelConfiguration, QuorumConfiguration
    <2>1b. /\ lockRank' =
                   [lockRank EXCEPT ![request.node] = request.qc.view]
            /\ lockSubject' =
                   [lockSubject EXCEPT ![request.node] = request.qc.subject]
      BY <1>1 DEF PersistLockCommit
    <2>2. ASSUME NEW vote \in commitIntents',
                  /\ vote.signer \in AsyncCurrentResponsiveVoters'
                     /\ vote.context = context'
           PROVE /\ vote.view <= lockRank'[vote.signer]
                 /\ (vote.view = lockRank'[vote.signer]
                       => vote.subject = lockSubject'[vote.signer])
      <3>1. /\ context' = context
             /\ AsyncCurrentResponsiveVoters' =
                  AsyncCurrentResponsiveVoters
             /\ commitIntents' = commitIntents \cup {request.vote}
        BY <1>1, Isa
           DEF PersistLockCommit, AsyncCurrentResponsiveVoters,
               CurrentVoters, CurrentEpoch
      <3>2. CASE vote = request.vote
        <4>1. /\ request.node \in Honest
               /\ request.vote.signer = request.node
               /\ request.vote.view = request.qc.view
               /\ request.vote.subject = request.qc.subject
               /\ request.qc.view >= lockRank[request.node]
               /\ (request.qc.view = lockRank[request.node]
                     => request.qc.subject = lockSubject[request.node])
          BY <1>1, <2>1 DEF PendingVoteWritesAuthorized
        <4>2. /\ lockRank'[request.node] = request.qc.view
               /\ lockSubject'[request.node] = request.qc.subject
          BY <2>1a, <2>1b, <4>1, ProgressFunctionalUpdateAtKey
        <4>3. /\ vote.view = lockRank'[vote.signer]
               /\ vote.subject = lockSubject'[vote.signer]
          BY <3>2, <4>1, <4>2
        <4> QED BY <4>3
      <3>3. CASE vote \in commitIntents
        <4>1. /\ vote.view <= lockRank[vote.signer]
               /\ (vote.view = lockRank[vote.signer]
                     => vote.subject = lockSubject[vote.signer])
          BY <1>1, <2>2, <3>1, <3>3
             DEF ResponsiveCommitIntentLockBound
        <4>2. CASE vote.signer = request.node
          <5>1. /\ request.node \in Honest
                 /\ request.qc.view >= lockRank[request.node]
                 /\ (request.qc.view = lockRank[request.node]
                       => request.qc.subject = lockSubject[request.node])
            BY <1>1, <2>1 DEF PendingVoteWritesAuthorized
          <5>2. /\ lockRank'[request.node] = request.qc.view
                 /\ lockSubject'[request.node] = request.qc.subject
            BY <2>1a, <2>1b, <5>1, ProgressFunctionalUpdateAtKey
          <5>3a. lockRank[vote.signer] <= request.qc.view
            BY <4>2, <5>1
          <5>3b. vote.view <= request.qc.view
            BY <4>1, <5>3a, WeakOrderTrans
          <5>3. vote.view <= lockRank'[vote.signer]
            BY <4>2, <5>2, <5>3b
          <5>4. ASSUME vote.view = lockRank'[vote.signer]
                 PROVE vote.subject = lockSubject'[vote.signer]
            <6>0. vote.view = request.qc.view
              BY <4>2, <5>2, <5>4
            <6>1a. /\ vote.view = lockRank[vote.signer]
                    /\ lockRank[vote.signer] = request.qc.view
              BY <4>1, <5>3a, <6>0, SqueezedOrderEquality
            <6>1. /\ vote.view = lockRank[vote.signer]
                   /\ request.qc.view = lockRank[request.node]
              BY <4>2, <5>1, <6>1a
            <6>2. /\ vote.subject = lockSubject[vote.signer]
                   /\ request.qc.subject = lockSubject[request.node]
              BY <4>1, <4>2, <5>1, <6>1
            <6> QED BY <4>2, <5>2, <6>2
          <5> QED BY <5>3, <5>4
        <4>3. CASE vote.signer # request.node
          <5>0a. TypeInvariant
            BY <1>1 DEF StrongInductiveInvariant, Safety
          <5>0b. AsyncCurrentResponsiveVoters \subseteq ValidatorIds
            BY <5>0a, AsyncCurrentResponsiveVotersAreValidators
          <5>0. vote.signer \in ValidatorIds
            BY <2>2, <3>1, <5>0b
          <5>1. /\ lockRank'[vote.signer] = lockRank[vote.signer]
                 /\ lockSubject'[vote.signer] = lockSubject[vote.signer]
            BY <1>1, <2>1a, <4>3, <5>0,
               FunctionalUpdateAwayFromKey
               DEF PersistLockCommit
          <5> QED BY <4>1, <5>1
        <4> QED BY <4>2, <4>3
      <3> QED BY <2>2, <3>1, <3>2, <3>3, SMT
    <2> QED BY <2>2 DEF ResponsiveCommitIntentLockBound
  <1> QED BY <1>1

THEOREM PersistInstallDoesNotActivateHigherResponsiveCommitIntent ==
  \A request \in pendingInstallTC, vote \in commitIntents:
    /\ ResponsiveCommitIntentLockBound
    /\ PersistInstallTC(request)
    /\ vote.signer \in AsyncCurrentResponsiveVoters
    /\ vote.context = context
    /\ lockRank'[vote.signer] > lockRank[vote.signer]
    => vote.view < lockRank'[vote.signer]
PROOF
  <1>1. ASSUME NEW request \in pendingInstallTC,
                NEW vote \in commitIntents,
                ResponsiveCommitIntentLockBound,
                PersistInstallTC(request),
                vote.signer \in AsyncCurrentResponsiveVoters,
                vote.context = context,
                lockRank'[vote.signer] > lockRank[vote.signer]
         PROVE vote.view < lockRank'[vote.signer]
    <2>1. vote.view <= lockRank[vote.signer]
      BY <1>1 DEF ResponsiveCommitIntentLockBound
    <2>2. lockRank[vote.signer] < lockRank'[vote.signer]
      BY <1>1
    <2> QED BY <2>1, <2>2, WeakStrictOrderTrans
  <1> QED BY <1>1

THEOREM PersistInstallPreservesResponsiveCommitIntentLockBound ==
  \A request \in pendingInstallTC:
    /\ StrongInductiveInvariant
    /\ ResponsiveCommitIntentLockBound
    /\ PersistInstallTC(request)
    => ResponsiveCommitIntentLockBound'
PROOF
  <1>1. ASSUME NEW request \in pendingInstallTC,
                StrongInductiveInvariant,
                ResponsiveCommitIntentLockBound,
                PersistInstallTC(request)
         PROVE ResponsiveCommitIntentLockBound'
    <2>1. ASSUME NEW vote \in commitIntents',
                  /\ vote.signer \in AsyncCurrentResponsiveVoters'
                     /\ vote.context = context'
           PROVE /\ vote.view <= lockRank'[vote.signer]
                 /\ (vote.view = lockRank'[vote.signer]
                       => vote.subject = lockSubject'[vote.signer])
      <3>1. /\ commitIntents' = commitIntents
             /\ context' = context
        BY <1>1, Isa DEF PersistInstallTC
      <3>2. AsyncCurrentResponsiveVoters' =
               AsyncCurrentResponsiveVoters
        BY <3>1 DEF AsyncCurrentResponsiveVoters,
                      CurrentVoters, CurrentEpoch
      <3>2a. /\ DOMAIN lockRank = ValidatorIds
              /\ DOMAIN lockSubject = ValidatorIds
        BY <1>1, Isa
           DEF StrongInductiveInvariant, Safety, TypeInvariant
      <3>2b. AsyncCurrentResponsiveVoters \subseteq ValidatorIds
        BY <1>1, AsyncCurrentResponsiveVotersAreValidators
           DEF StrongInductiveInvariant, Safety
      <3>2c. TypeInvariant
        BY <1>1 DEF StrongInductiveInvariant, Safety
      <3>3. lockRank'[vote.signer] >= lockRank[vote.signer]
        <4>1. LockMonotonicityAction
          BY <1>1, <3>2c, PersistInstallTCIsLockMonotone
        <4>2. vote.signer \in ValidatorIds
          BY <2>1, <3>2, <3>2b
        <4> QED BY <3>1, <4>1, <4>2
             DEF LockMonotonicityAction
      <3>4. /\ vote.view <= lockRank[vote.signer]
             /\ (vote.view = lockRank[vote.signer]
                   => vote.subject = lockSubject[vote.signer])
        BY <1>1, <2>1, <3>1, <3>2
           DEF ResponsiveCommitIntentLockBound
      <3>5. CASE lockRank'[vote.signer] = lockRank[vote.signer]
        <4>1. lockSubject'[vote.signer] = lockSubject[vote.signer]
          <5>1. CASE vote.signer = request.node
            <6>1. vote.signer \in ValidatorIds
              BY <2>1, <3>2, <3>2b
            <6>2. ~(TcHighRank(request.tc) > lockRank[request.node])
              BY <1>1, <3>2a, <3>5, <5>1, <6>1, Isa
                 DEF PersistInstallTC
            <6> QED BY <1>1, <3>2a, <5>1, <6>1, <6>2, Isa
                 DEF PersistInstallTC
          <5>2. CASE vote.signer # request.node
            <6>1. vote.signer \in ValidatorIds
              BY <2>1, <3>2, <3>2b
            <6> QED BY <1>1, <3>2a, <5>2, <6>1,
                 FunctionalUpdateAwayFromKey
                 DEF PersistInstallTC
          <5> QED BY <5>1, <5>2
        <4> QED BY <3>4, <3>5, <4>1
      <3>6. CASE lockRank'[vote.signer] > lockRank[vote.signer]
        <4>1. vote.view < lockRank'[vote.signer]
          BY <1>1, <2>1, <3>1, <3>2, <3>6,
             PersistInstallDoesNotActivateHigherResponsiveCommitIntent
        <4> QED BY <4>1
      <3> QED BY <3>3, <3>5, <3>6, SMT
    <2> QED BY <2>1 DEF ResponsiveCommitIntentLockBound
  <1> QED BY <1>1

THEOREM AsyncNextPreservesResponsiveCommitIntentLockBound ==
  /\ StrongInductiveInvariant
  /\ ResponsiveCommitIntentLockBound
  /\ AsyncNext
  => ResponsiveCommitIntentLockBound'
PROOF
  <1>1. ASSUME StrongInductiveInvariant,
              ResponsiveCommitIntentLockBound,
              AsyncNext
         PROVE ResponsiveCommitIntentLockBound'
    <2>1. [Next]_vars
      BY <1>1, AsyncStepRefinementObligation
    <2>2. CASE UNCHANGED vars
      BY <1>1, <2>2, Isa
         DEF ResponsiveCommitIntentLockBound, vars,
             AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch
    <2>3. CASE Next
      <3>1. CASE LockStableNext
        BY <1>1, <3>1, LockStableNextLeavesCommitLockFrame, Isa
           DEF ResponsiveCommitIntentLockBound,
               AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch
      <3>2. CASE \E request \in pendingLockCommit:
                    PersistLockCommit(request)
        BY <1>1, <3>2,
           PersistLockPreservesResponsiveCommitIntentLockBound
      <3>3. CASE \E request \in pendingInstallTC:
                    PersistInstallTC(request)
        BY <1>1, <3>3,
           PersistInstallPreservesResponsiveCommitIntentLockBound
      <3> QED BY <2>3, <3>1, <3>2, <3>3,
           NextLockFootprintClassification
    <2> QED BY <2>1, <2>2, <2>3
  <1> QED BY <1>1

THEOREM RememberedNonCommitControlPreservesCommitItems ==
  \A retained, items:
    (\A item \in items: item.kind # "CommitVote")
      => {item \in RememberedControl(retained, items):
             item.kind = "CommitVote"}
           = {item \in retained: item.kind = "CommitVote"}
BY IsaM("blast")
   DEF RememberedControl, RetainedClassItems, ControlClass

THEOREM InstalledNonCommitControlPreservesCommitItems ==
  \A retained, node, items:
    (\A item \in items: item.kind # "CommitVote")
      => {item \in InstalledControl(retained, node, items):
             item.kind = "CommitVote"}
           = {item \in retained: item.kind = "CommitVote"}
BY RememberedNonCommitControlPreservesCommitItems, Isa
   DEF InstalledControl, ControlClass,
       AsyncInstallRetainedControlKinds

THEOREM ExecuteRegularCommandHasCommitSourceTransition ==
  \A command:
    ExecuteRegularCommand(command)
      => \/ UNCHANGED CommitCoreSourceProjection
         \/ \E request \in pendingLockCommit:
              PersistLockCommitSourceTransition(request)
BY IsaM("blast")
   DEF ExecuteRegularCommand, RegularCoreCommand,
       PersistLockCommitSourceTransition, CommitCoreSourceProjection,
       ResponsiveCommitSignRequests, ResponsiveRetainedCommitItems,
       AsyncAuxVars, AssembleLocalBody, BeginLocalProposal,
       PersistProposal, FetchBody, RebindRetainedBody, StoreBody,
       ValidateBody, ValidateDecidedBody, ValidateLockedBody, RejectBody,
       BeginPrepare,
       PersistPrepare, BeginObservePrepare, PersistObservePrepare,
       BeginLockCommit, PersistLockCommit, FormCommitQC, BeginDecision,
       PersistTimeout, BeginInstallTC, FetchCertifiedBody, vars

THEOREM ExecutePersistInstallHasCommitSourceTransition ==
  \A command:
    ExecutePersistInstall(command)
      => \E request \in pendingInstallTC:
           PersistInstallCommitSourceTransition(request)
BY InstalledNonCommitControlPreservesCommitItems, IsaM("blast")
   DEF ExecutePersistInstall, PersistInstalledControl,
       PersistInstalledControlAfterInstall,
       PersistInstallCommitSourceTransition,
       ResponsiveCommitSignRequests, ResponsiveRetainedCommitItems,
       TcOutbox

THEOREM ExecuteNonVoteCommandPreservesCommitCoreSourceProjection ==
  \A command:
    /\ ~ExecuteRegularCommand(command)
    /\ ~ExecuteSignVote(command)
    /\ ~ExecutePersistInstall(command)
    /\ ExecuteCommand(command)
    => UNCHANGED CommitCoreSourceProjection
BY RememberedNonCommitControlPreservesCommitItems, IsaM("blast")
   DEF ExecuteCommand, ExecuteSignProposal, ExecuteFormPrepareQC,
       ExecuteSignTimeout, ExecutePersistDecision,
       ExecuteRequestCertifiedBody, ExecuteApply, ExecuteCoreDelivery,
       ExecuteChunkDelivery, ExecuteRejectAuthenticatedJunk,
       CompleteProposalSignature, FormPrepareQC,
       CompleteTimeoutSignature, PersistDecision,
       PublishControlItems, PublishControlAndEphemeralItems,
       PersistDecisionControl, QcOutbox, ProposalOutbox,
       TimeoutOutbox, VoteOutbox, RememberedControl,
       CommitCoreSourceProjection, ResponsiveCommitSignRequests,
       ResponsiveRetainedCommitItems, vars

THEOREM ExecuteCommandHasCommitSourceTransition ==
  \A command:
    ExecuteCommand(command)
      => \/ UNCHANGED CommitCoreSourceProjection
         \/ ExecuteSignVote(command)
         \/ \E request \in pendingLockCommit:
              PersistLockCommitSourceTransition(request)
         \/ \E request \in pendingInstallTC:
              PersistInstallCommitSourceTransition(request)
PROOF
  <1>1. ASSUME NEW command, ExecuteCommand(command)
         PROVE \/ UNCHANGED CommitCoreSourceProjection
               \/ ExecuteSignVote(command)
               \/ \E request \in pendingLockCommit:
                    PersistLockCommitSourceTransition(request)
               \/ \E request \in pendingInstallTC:
                    PersistInstallCommitSourceTransition(request)
    <2>1. CASE ExecuteRegularCommand(command)
      BY <2>1, ExecuteRegularCommandHasCommitSourceTransition
    <2>2. CASE ExecuteSignVote(command)
      BY <2>2
    <2>3. CASE ExecutePersistInstall(command)
      BY <2>3, ExecutePersistInstallHasCommitSourceTransition
    <2>4. CASE /\ ~ExecuteRegularCommand(command)
                 /\ ~ExecuteSignVote(command)
                 /\ ~ExecutePersistInstall(command)
      BY <1>1, <2>4,
         ExecuteNonVoteCommandPreservesCommitCoreSourceProjection
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4
         DEF ExecuteCommand
  <1> QED BY <1>1

THEOREM FifoRuntimeHasCommitSourceTransition ==
  \A node:
    FifoRuntimeStep(node)
      => \/ UNCHANGED CommitCoreSourceProjection
         \/ \E command: ExecuteSignVote(command)
         \/ \E request \in pendingLockCommit:
              PersistLockCommitSourceTransition(request)
         \/ \E request \in pendingInstallTC:
              PersistInstallCommitSourceTransition(request)
PROOF
  <1>1. ASSUME NEW node, FifoRuntimeStep(node)
         PROVE \/ UNCHANGED CommitCoreSourceProjection
               \/ \E command: ExecuteSignVote(command)
               \/ \E request \in pendingLockCommit:
                    PersistLockCommitSourceTransition(request)
               \/ \E request \in pendingInstallTC:
                    PersistInstallCommitSourceTransition(request)
    <2> DEFINE Command == NextNodeCommand(node)
    <2>1. CASE CommandDispatchable(Command)
      <3>1. ExecuteCommand(Command)
        BY <1>1, <2>1, Isa DEF FifoRuntimeStep, Command
      <3> QED BY <3>1, ExecuteCommandHasCommitSourceTransition
    <2>2. CASE ~CommandDispatchable(Command)
      BY <1>1, <2>2, Isa
         DEF FifoRuntimeStep, DeferCommand, DiscardCommand,
             CommitCoreSourceProjection, ResponsiveCommitSignRequests,
             ResponsiveRetainedCommitItems, Command, vars
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM DeferredDrainHasCommitSourceTransition ==
  \A node:
    DeferredDrainStep(node)
      => \/ UNCHANGED CommitCoreSourceProjection
         \/ \E command: ExecuteSignVote(command)
         \/ \E request \in pendingLockCommit:
              PersistLockCommitSourceTransition(request)
         \/ \E request \in pendingInstallTC:
              PersistInstallCommitSourceTransition(request)
PROOF
  <1>1. ASSUME NEW node, DeferredDrainStep(node)
         PROVE \/ UNCHANGED CommitCoreSourceProjection
               \/ \E command: ExecuteSignVote(command)
               \/ \E request \in pendingLockCommit:
                    PersistLockCommitSourceTransition(request)
               \/ \E request \in pendingInstallTC:
                    PersistInstallCommitSourceTransition(request)
    <2>1. CASE ~DeferredQueueNonempty(node)
      BY <1>1, <2>1, Isa
         DEF DeferredDrainStep, DeferredWorkServiceable,
             CommitCoreSourceProjection,
             ResponsiveCommitSignRequests,
             ResponsiveRetainedCommitItems, vars
    <2>2. CASE DeferredQueueNonempty(node)
      <3> DEFINE Command == NextDeferredCommand(node)
      <3>1. CASE DeferredHandoffAllowsExecution(node, Command)
        <4>1. ExecuteCommand(Command)
          BY <1>1, <2>2, <3>1, Isa
             DEF DeferredDrainStep, Command
        <4> QED BY <4>1, ExecuteCommandHasCommitSourceTransition
      <3>2. CASE ~DeferredHandoffAllowsExecution(node, Command)
        BY <1>1, <2>2, <3>2, Isa
           DEF DeferredDrainStep, DiscardCommand,
               CommitCoreSourceProjection, ResponsiveCommitSignRequests,
               ResponsiveRetainedCommitItems, Command, vars
      <3> QED BY <3>1, <3>2
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM RuntimeStepHasCommitSourceTransition ==
  \A node:
    RuntimeStep(node)
      => \/ UNCHANGED CommitCoreSourceProjection
         \/ \E command: ExecuteSignVote(command)
         \/ \E request \in pendingLockCommit:
              PersistLockCommitSourceTransition(request)
         \/ \E request \in pendingInstallTC:
              PersistInstallCommitSourceTransition(request)
PROOF
  <1>1. ASSUME NEW node, RuntimeStep(node)
         PROVE \/ UNCHANGED CommitCoreSourceProjection
               \/ \E command: ExecuteSignVote(command)
               \/ \E request \in pendingLockCommit:
                    PersistLockCommitSourceTransition(request)
               \/ \E request \in pendingInstallTC:
                    PersistInstallCommitSourceTransition(request)
    <2>1. CASE FifoRuntimeStep(node)
      BY <2>1, FifoRuntimeHasCommitSourceTransition
    <2>2. CASE DeferredDrainStep(node)
      BY <2>2, DeferredDrainHasCommitSourceTransition
    <2>3. CASE NonQueueRuntimeAction(node)
      BY <2>3, Isa
         DEF NonQueueRuntimeAction,
             DeferredTagStep, DeferredTimeoutStep,
             DeferredRetransmitStep, DirectTimeoutStep,
             DirectRetransmitStep, IdleRuntimeStep, BeginTimeout,
             SendNodeRetransmissions, NoSendItem,
             PublishEphemeralItems, LeaveCausalQueues,
             CommitCoreSourceProjection, ResponsiveCommitSignRequests,
             ResponsiveRetainedCommitItems, vars
    <2>4. \/ FifoRuntimeStep(node)
           \/ DeferredDrainStep(node)
           \/ NonQueueRuntimeAction(node)
      BY <1>1 DEF RuntimeStep, NonQueueRuntimeAction
    <2> QED BY <2>1, <2>2, <2>3, <2>4
  <1> QED BY <1>1

THEOREM ReplayRunNodeContinuationHasCommitSourceTransition ==
  \A node:
    ReplayRunNodeCandidateProducerContinuation(node)
      => \/ UNCHANGED CommitCoreSourceProjection
         \/ \E command: ExecuteSignVote(command)
         \/ \E request \in pendingLockCommit:
              PersistLockCommitSourceTransition(request)
         \/ \E request \in pendingInstallTC:
              PersistInstallCommitSourceTransition(request)
PROOF
  <1>1. ASSUME NEW node,
                ReplayRunNodeCandidateProducerContinuation(node)
         PROVE \/ UNCHANGED CommitCoreSourceProjection
               \/ \E command: ExecuteSignVote(command)
               \/ \E request \in pendingLockCommit:
                    PersistLockCommitSourceTransition(request)
               \/ \E request \in pendingInstallTC:
                    PersistInstallCommitSourceTransition(request)
    <2>1. CASE
              AsyncCandidateProducerContinuationExactLocalReplayStep(node)
      BY <2>1, Isa
         DEF AsyncCandidateProducerContinuationExactLocalReplayStep,
             CommitCoreSourceProjection, ResponsiveCommitSignRequests,
             ResponsiveRetainedCommitItems, vars
    <2>2. CASE
              AsyncCandidateProducerContinuationReplayTargetOnlyTurn(node)
      BY <2>2, Isa
         DEF AsyncCandidateProducerContinuationReplayTargetOnlyTurn,
             CommitCoreSourceProjection, ResponsiveCommitSignRequests,
             ResponsiveRetainedCommitItems, vars
    <2>3. CASE
              AsyncCandidateProducerContinuationExactRuntimeReplayStep(node)
      <3>1. RuntimeStep(node)
        BY <2>3, Isa
           DEF AsyncCandidateProducerContinuationExactRuntimeReplayStep,
               RuntimeStep
      <3> QED BY <3>1, RuntimeStepHasCommitSourceTransition
    <2> QED BY <1>1, <2>1, <2>2, <2>3
         DEF ReplayRunNodeCandidateProducerContinuation
  <1> QED BY <1>1

THEOREM RunNodeWorkHasCommitSourceTransition ==
  \A node:
    RunNodeWork(node)
      => \/ UNCHANGED CommitCoreSourceProjection
         \/ \E command: ExecuteSignVote(command)
         \/ \E request \in pendingLockCommit:
              PersistLockCommitSourceTransition(request)
         \/ \E request \in pendingInstallTC:
              PersistInstallCommitSourceTransition(request)
PROOF
  <1>1. ASSUME NEW node, RunNodeWork(node)
         PROVE \/ UNCHANGED CommitCoreSourceProjection
               \/ \E command: ExecuteSignVote(command)
               \/ \E request \in pendingLockCommit:
                    PersistLockCommitSourceTransition(request)
               \/ \E request \in pendingInstallTC:
                    PersistInstallCommitSourceTransition(request)
    <2>0. CASE
            ResolveRunNodeCandidateProducerContinuation(node)
      BY <2>0, Isa
         DEF ResolveRunNodeCandidateProducerContinuation,
             CommitCoreSourceProjection, vars
    <2>0p. CASE
             ReplayRunNodeCandidateProducerContinuation(node)
      BY <2>0p, ReplayRunNodeContinuationHasCommitSourceTransition
    <2>1. CASE LocalAdmissionStep(node)
      BY <1>1, <2>1, Isa
         DEF LocalAdmissionStep, AdmitProducerCompletion,
             AdmitCausalHead, UpdateLocalAdmissionMetadata,
             CommitCoreSourceProjection,
             ResponsiveCommitSignRequests,
             ResponsiveRetainedCommitItems, vars
    <2>2. CASE IngressDrainStep(node)
      BY <1>1, <2>2, Isa
         DEF IngressDrainStep, DrainFairIngressSelected,
             CommitCoreSourceProjection, ResponsiveCommitSignRequests,
             ResponsiveRetainedCommitItems, vars
    <2>3. CASE SerializedRuntimeStep(node)
                  \/ SerializedRuntimePrecedesServeIngressStep(node)
      <3>1. RuntimeStep(node)
        BY <2>3, Isa
           DEF SerializedRuntimeStep,
               SerializedRuntimePrecedesServeIngressStep
      <3> QED BY <3>1, RuntimeStepHasCommitSourceTransition
    <2>4. CASE AsyncServeIngressTargetOnlyTurn(node)
      BY <2>4, Isa
         DEF AsyncServeIngressTargetOnlyTurn,
             CommitCoreSourceProjection,
             ResponsiveCommitSignRequests,
             ResponsiveRetainedCommitItems, vars
    <2>5. CASE SerializedLocalPrecedesServeIngressStep(node)
      BY <2>5, Isa
         DEF SerializedLocalPrecedesServeIngressStep,
             SelectedLocalAdmissionAdvance,
             AdmitProducerCompletion, AdmitCausalHead,
             UpdateLocalAdmissionMetadata,
             CommitCoreSourceProjection,
             ResponsiveCommitSignRequests,
             ResponsiveRetainedCommitItems, vars
    <2> QED BY <1>1, <2>0, <2>0p, <2>1, <2>2, <2>3, <2>4,
                 <2>5
         DEF RunNodeWork
  <1> QED BY <1>1

THEOREM AsyncNextHasCommitSourceTransition ==
  AsyncNext
    => \/ UNCHANGED CommitSourceProjection
       \/ \E command: ExecuteSignVote(command)
       \/ \E request \in pendingLockCommit:
            PersistLockCommitSourceTransition(request)
       \/ \E request \in pendingInstallTC:
            PersistInstallCommitSourceTransition(request)
       \/ \E node \in ValidatorIds:
            ResponsiveCrashCommitSourceTransition(node)
       \/ ResponsiveRestartCommitSourceTransition
       \/ ResponsiveReplayCommitSourceTransition
       \/ ResponsiveReplayContinuationCommitSourceTransition
PROOF
  <1>1. ASSUME AsyncNext
         PROVE \/ UNCHANGED CommitSourceProjection
               \/ \E command: ExecuteSignVote(command)
               \/ \E request \in pendingLockCommit:
                    PersistLockCommitSourceTransition(request)
               \/ \E request \in pendingInstallTC:
                    PersistInstallCommitSourceTransition(request)
               \/ \E node \in ValidatorIds:
                    ResponsiveCrashCommitSourceTransition(node)
               \/ ResponsiveRestartCommitSourceTransition
               \/ ResponsiveReplayCommitSourceTransition
               \/ ResponsiveReplayContinuationCommitSourceTransition
    <2>1. CASE AsyncNonCrashStep
      <3>1. CASE ResponsiveReplayContinuationCommitSourceTransition
        BY <3>1
      <3>2. CASE ~ResponsiveReplayContinuationCommitSourceTransition
        <4>1. UNCHANGED AsyncRecoveryControlVars
          BY <2>1, <3>2
             DEF AsyncNonCrashStep,
                 ResponsiveReplayContinuationCommitSourceTransition
        <4>2. CASE \E node \in AsyncCurrentResponsiveVoters:
                      RunNode(node)
          <5>1. ASSUME NEW node \in AsyncCurrentResponsiveVoters,
                        RunNode(node)
                 PROVE \/ UNCHANGED CommitSourceProjection
                       \/ \E command: ExecuteSignVote(command)
                       \/ \E request \in pendingLockCommit:
                            PersistLockCommitSourceTransition(request)
                       \/ \E request \in pendingInstallTC:
                            PersistInstallCommitSourceTransition(request)
                       \/ \E crashNode \in ValidatorIds:
                            ResponsiveCrashCommitSourceTransition(crashNode)
                       \/ ResponsiveRestartCommitSourceTransition
                       \/ ResponsiveReplayCommitSourceTransition
                       \/ ResponsiveReplayContinuationCommitSourceTransition
            BY <4>1, <5>1, RunNodeWorkHasCommitSourceTransition, Isa
               DEF CommitSourceProjection
          <5> QED BY <4>2, <5>1
        <4>3. CASE \E node \in asyncHistoricalRecoveryTargets:
                      RunHistoricalRecoveryNode(node)
          <5>1. ASSUME NEW node \in asyncHistoricalRecoveryTargets,
                        RunHistoricalRecoveryNode(node)
                 PROVE \/ UNCHANGED CommitSourceProjection
                       \/ \E command: ExecuteSignVote(command)
                       \/ \E request \in pendingLockCommit:
                            PersistLockCommitSourceTransition(request)
                       \/ \E request \in pendingInstallTC:
                            PersistInstallCommitSourceTransition(request)
                       \/ \E crashNode \in ValidatorIds:
                            ResponsiveCrashCommitSourceTransition(crashNode)
                       \/ ResponsiveRestartCommitSourceTransition
                       \/ ResponsiveReplayCommitSourceTransition
                       \/ ResponsiveReplayContinuationCommitSourceTransition
            BY <4>1, <5>1, RunNodeWorkHasCommitSourceTransition, Isa
               DEF RunHistoricalRecoveryNode, CommitSourceProjection
          <5> QED BY <4>3, <5>1
        <4>4. CASE /\ ~(\E node \in AsyncCurrentResponsiveVoters:
                            RunNode(node))
                     /\ ~(\E node \in asyncHistoricalRecoveryTargets:
                            RunHistoricalRecoveryNode(node))
          BY <1>1, <2>1, <3>2, <4>1, <4>4, IsaM("blast")
             DEF AsyncNext, AsyncNonCrashStep, AsyncRunnerStep,
                 AsyncNonRunnerStep, RunHistoricalServer,
                 DrainHistoricalIngressSelected, HistoricalIdleStep,
                 AsyncSetGST, AsyncTick, OpenHistoricalRecovery,
                 DirectCommitCertificateDiscoveryStep,
                 DirectHistoricalCommitCertificateDiscoveryStep,
                 CommitCertificateDiscoveryStepWork,
                 ServiceIoWorker, ServiceHistoricalRecoveryIoWorker,
                 ServiceIoWorkerWork,
                 EnqueueIoLocalControl,
                 EnqueueHistoricalRecoveryIoLocalControl,
                 EnqueueIoLocalControlWork, AsyncNetworkStep,
                 AdmitIngressPacket, AdmitHiddenPacket,
                 CoalesceHiddenPacket, AsyncFaultStep, PreGstLosePacket,
                 PreGstCrash, Crash, InjectByzantineNoise,
                 InjectUntrustedTransportCompletion,
                 InjectAuthenticatedJunk,
                 InjectByzantineCertifiedRequest,
                 AsyncByzantineProposal, AsyncByzantineVote,
                 AsyncByzantineTimeout, PublishEphemeralItems,
                 CommitCoreSourceProjection, CommitSourceProjection,
                 ResponsiveCommitSignRequests,
                 ResponsiveRetainedCommitItems,
                 ResponsiveReplayContinuationCommitSourceTransition,
                 DriveResponsiveReplayHead, FinishResponsiveReplay,
                 RearmResponsiveRecovery,
                 AsyncSchedulerVars, AsyncNonClockVars, vars
        <4> QED BY <4>2, <4>3, <4>4
      <3> QED BY <3>1, <3>2
    <2>2. CASE \E node \in ValidatorIds: PreGstCrash(node)
      BY <2>2, Isa
         DEF PreGstCrash, Crash, CommitCoreSourceProjection,
             CommitSourceProjection, ResponsiveCommitSignRequests,
             ResponsiveRetainedCommitItems, AsyncSchedulerVars
    <2>3. CASE \E node \in ValidatorIds:
                  ResponsiveCrashCommitSourceTransition(node)
      BY <2>3
    <2>4. CASE ResponsiveRestartCommitSourceTransition
      BY <2>4
    <2>5. CASE ResponsiveReplayCommitSourceTransition
      BY <2>5
    <2>6. CASE ResponsiveReplayContinuationCommitSourceTransition
      BY <2>6
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4, <2>5, <2>6
         DEF AsyncNext, ResponsiveCrashCommitSourceTransition,
             ResponsiveRestartCommitSourceTransition,
             ResponsiveReplayCommitSourceTransition,
             ResponsiveReplayContinuationCommitSourceTransition
  <1> QED BY <1>1

THEOREM CommitSourceProjectionFramePreservesAuxiliaryInvariant ==
  /\ CommitSourceAuxiliaryInvariant
  /\ UNCHANGED CommitSourceProjection
  => CommitSourceAuxiliaryInvariant'
BY Isa
   DEF CommitSourceAuxiliaryInvariant,
       ResponsiveRetainedCommitControlSound,
       ActiveCommitIntentSourceOwnership,
       AsyncCommitIntentProgressWitness, CommitIntentProgressWitness,
       CommitRecoveryAuthority,
       ActiveLockedCommitIntent, RetainedCommitIntent,
       AsyncCommitIntentProgressWitness, CommitIntentProgressWitness,
       CommitRecoveryAuthority,
       CommitSourceProjection, ResponsiveCommitSignRequests,
       ResponsiveRetainedCommitItems,
       AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch

THEOREM PersistLockPreservesCommitSourceAuxiliaryInvariant ==
  \A request \in pendingLockCommit:
    /\ AsyncStrongTypeInvariant
    /\ CommitSourceRetentionInvariant
    /\ PersistLockCommitSourceTransition(request)
    => CommitSourceAuxiliaryInvariant'
BY SMTT(90), Isa
   DEF AsyncStrongTypeInvariant, AsyncSchedulerTypeInvariant,
       StrongInductiveInvariant, Safety, TypeInvariant,
       ReducerProvenanceInvariant, PendingVoteWritesAuthorized,
       HonestVoteUnique, HonestCommitUniqueness,
       AllPendingRequests,
       CommitSourceRetentionInvariant, CommitSourceAuxiliaryInvariant,
       ResponsiveCommitIntentLockBound,
       ResponsiveRetainedCommitControlSound,
       ActiveCommitIntentSourceOwnership,
       AsyncCommitIntentProgressWitness, CommitIntentProgressWitness,
       CommitRecoveryAuthority,
       ActiveLockedCommitIntent, RetainedCommitIntent,
       PersistLockCommitSourceTransition, PersistLockCommit,
       ResponsiveRetainedCommitItems, VoteSign,
       AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch

THEOREM ExecuteSignVotePreservesCommitSourceAuxiliaryInvariant ==
  \A command:
    /\ AsyncStrongTypeInvariant
    /\ CommitSourceRetentionInvariant
    /\ ExecuteSignVote(command)
    => CommitSourceAuxiliaryInvariant'
BY SMTT(120), Isa
   DEF AsyncStrongTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncTransportTypeInvariant,
       AsyncTransportContentTypeInvariant,
       AsyncTransportHistoryTypeInvariant,
       StrongInductiveInvariant, Safety, TypeInvariant,
       ReducerProvenanceInvariant, HonestVoteUnique,
       HonestCommitUniqueness, CommitSigningRequiresIntent,
       CommitSourceRetentionInvariant, CommitSourceAuxiliaryInvariant,
       ResponsiveCommitIntentLockBound,
       ResponsiveRetainedCommitControlSound,
       ActiveCommitIntentSourceOwnership,
       AsyncCommitIntentProgressWitness, CommitIntentProgressWitness,
       CommitRecoveryAuthority,
       ActiveLockedCommitIntent, RetainedCommitIntent,
       ExecuteSignVote, CompleteVoteSignature, PublishControlItems,
       RememberedControl, RetainedClassItems, ControlClass, ControlView,
       VoteOutbox, VoteSign, AsyncNetworkItem, VoteEnvelope,
       AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch,
       vars

THEOREM PersistInstallPreservesCommitSourceAuxiliaryInvariant ==
  \A request \in pendingInstallTC:
    /\ AsyncStrongTypeInvariant
    /\ CommitSourceRetentionInvariant
    /\ PersistInstallCommitSourceTransition(request)
    => CommitSourceAuxiliaryInvariant'
PROOF
  <1>1. ASSUME NEW request \in pendingInstallTC,
                AsyncStrongTypeInvariant,
                CommitSourceRetentionInvariant,
                PersistInstallCommitSourceTransition(request)
         PROVE CommitSourceAuxiliaryInvariant'
    <2>1. /\ ResponsiveCommitIntentLockBound
           /\ CommitSourceAuxiliaryInvariant
      BY <1>1 DEF CommitSourceRetentionInvariant
    <2>2. \A vote \in commitIntents:
             /\ vote.signer \in AsyncCurrentResponsiveVoters
             /\ vote.context = context
             /\ lockRank'[vote.signer] > lockRank[vote.signer]
            => vote.view < lockRank'[vote.signer]
      BY <1>1, <2>1,
         PersistInstallDoesNotActivateHigherResponsiveCommitIntent
         DEF PersistInstallCommitSourceTransition
    <2> QED BY <1>1, <2>1, <2>2, Isa
         DEF CommitSourceAuxiliaryInvariant,
             ResponsiveRetainedCommitControlSound,
             ActiveCommitIntentSourceOwnership,
             AsyncCommitIntentProgressWitness,
             CommitIntentProgressWitness, CommitRecoveryAuthority,
             ActiveLockedCommitIntent, RetainedCommitIntent,
             PersistInstallCommitSourceTransition, PersistInstallTC,
             ResponsiveCommitSignRequests,
             ResponsiveRetainedCommitItems,
             AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch
  <1> QED BY <1>1

THEOREM ResponsiveCrashPreservesCommitSourceAuxiliaryInvariant ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ CommitSourceRetentionInvariant
    /\ ResponsiveCrashCommitSourceTransition(node)
    => CommitSourceAuxiliaryInvariant'
BY SMTT(60), Isa
   DEF ResponsiveCrashCommitSourceTransition,
       PreGstResponsiveCrash, Crash,
       AsyncStrongTypeInvariant, AsyncRecoveryTypeInvariant,
       AsyncRestartAuthorityInvariant,
       CommitSourceRetentionInvariant, CommitSourceAuxiliaryInvariant,
       ResponsiveRetainedCommitControlSound,
       ActiveCommitIntentSourceOwnership,
       AsyncCommitIntentProgressWitness, CommitIntentProgressWitness,
       CommitRecoveryAuthority, ActiveLockedCommitIntent,
       RetainedCommitIntent, NodeHasDecision,
       AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch

THEOREM ResponsiveRestartPreservesCommitSourceAuxiliaryInvariant ==
  /\ AsyncStrongTypeInvariant
  /\ CommitSourceRetentionInvariant
  /\ ResponsiveRestartCommitSourceTransition
  => CommitSourceAuxiliaryInvariant'
BY SMTT(60), Isa
   DEF ResponsiveRestartCommitSourceTransition,
       PreGstResponsiveRestart, Restart,
       AsyncStrongTypeInvariant, AsyncRecoveryTypeInvariant,
       AsyncRestartAuthorityInvariant,
       CommitSourceRetentionInvariant, CommitSourceAuxiliaryInvariant,
       ResponsiveRetainedCommitControlSound,
       ActiveCommitIntentSourceOwnership,
       AsyncCommitIntentProgressWitness, CommitIntentProgressWitness,
       CommitRecoveryAuthority, ActiveLockedCommitIntent,
       RetainedCommitIntent, NodeHasDecision,
       AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch

THEOREM ResponsiveReplayPreservesCommitSourceAuxiliaryInvariant ==
  /\ AsyncStrongTypeInvariant
  /\ CommitSourceRetentionInvariant
  /\ ResponsiveReplayCommitSourceTransition
  => CommitSourceAuxiliaryInvariant'
BY SMTT(120), Isa
   DEF ResponsiveReplayCommitSourceTransition,
       PreGstResponsiveReplay, RecoveryCoreReplay,
       ResetNodeSchedulerForRestart, RestartReplay,
       RestartDecisions, RestartLockedCommitIntents,
       RestartTimeoutIntents, RestartPrepareIntents,
       RestartProposalIntents,
       ResumeProposal, ResumeVote, ResumeTimeout, VoteResumeAuthorized,
       AsyncStrongTypeInvariant, AsyncRecoveryTypeInvariant,
       AsyncRestartAuthorityInvariant,
       StrongInductiveInvariant, Safety, TypeInvariant,
       HonestVoteUnique, HonestCommitUniqueness,
       CommitSourceRetentionInvariant, CommitSourceAuxiliaryInvariant,
       ResponsiveRetainedCommitControlSound,
       ActiveCommitIntentSourceOwnership,
       AsyncCommitIntentProgressWitness, CommitIntentProgressWitness,
       CommitRecoveryAuthority, ActiveLockedCommitIntent,
       RetainedCommitIntent, NodeHasDecision, VoteSign,
       AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch

THEOREM ReplayCommitIntentReadyIsProgressWitness ==
  \A node, vote:
    ReplayCommitIntentReady(node, vote)
      <=> CommitIntentProgressWitness(node, vote)
BY Isa
   DEF ReplayCommitIntentReady, CommitIntentProgressWitness,
       RetainedCommitIntent

THEOREM DriveResponsiveReplayPreservesCommitSourceAuxiliaryInvariant ==
  /\ AsyncStrongTypeInvariant
  /\ CommitSourceRetentionInvariant
  /\ DriveResponsiveReplayHead
  => CommitSourceAuxiliaryInvariant'
BY RestartSignatureReplayProperties, SMTT(90), Isa
   DEF DriveResponsiveReplayHead, RecoveryCoreReplay,
       ResumeProposal, ResumeVote, ResumeTimeout, VoteResumeAuthorized,
       AsyncStrongTypeInvariant, AsyncRecoveryTypeInvariant,
       AsyncRestartAuthorityInvariant,
       StrongInductiveInvariant, Safety, TypeInvariant,
       HonestVoteUnique, HonestCommitUniqueness,
       CommitSourceRetentionInvariant, CommitSourceAuxiliaryInvariant,
       ResponsiveRetainedCommitControlSound,
       ActiveCommitIntentSourceOwnership,
       AsyncCommitIntentProgressWitness, CommitIntentProgressWitness,
       CommitRecoveryAuthority, ActiveLockedCommitIntent,
       RetainedCommitIntent, NodeHasDecision, VoteSign,
       ResponsiveReplayScheduledCandidates,
       AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch

THEOREM FinishResponsiveReplayPreservesCommitSourceAuxiliaryInvariant ==
  /\ AsyncStrongTypeInvariant
  /\ CommitSourceRetentionInvariant
  /\ FinishResponsiveReplay
  => CommitSourceAuxiliaryInvariant'
BY ReplayCommitIntentReadyIsProgressWitness, SMTT(120), Isa
   DEF FinishResponsiveReplay, ReplayCommitSourcesReady,
       RestartLockedCommitIntents,
       AsyncStrongTypeInvariant, AsyncRecoveryTypeInvariant,
       AsyncRestartAuthorityInvariant,
       StrongInductiveInvariant, Safety, TypeInvariant,
       HonestVoteUnique, HonestCommitUniqueness,
       CommitSourceRetentionInvariant, CommitSourceAuxiliaryInvariant,
       ResponsiveRetainedCommitControlSound,
       ActiveCommitIntentSourceOwnership,
       AsyncCommitIntentProgressWitness, CommitIntentProgressWitness,
       CommitRecoveryAuthority, ActiveLockedCommitIntent,
       RetainedCommitIntent, NodeHasDecision, VoteSign,
       ResponsiveReplayScheduledCandidates,
       AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch

THEOREM RearmResponsiveRecoveryPreservesCommitSourceAuxiliaryInvariant ==
  /\ AsyncStrongTypeInvariant
  /\ CommitSourceRetentionInvariant
  /\ RearmResponsiveRecovery
  => CommitSourceAuxiliaryInvariant'
BY SMTT(60), Isa
   DEF RearmResponsiveRecovery,
       AsyncStrongTypeInvariant, AsyncRecoveryTypeInvariant,
       AsyncRestartAuthorityInvariant,
       CommitSourceRetentionInvariant, CommitSourceAuxiliaryInvariant,
       ResponsiveRetainedCommitControlSound,
       ActiveCommitIntentSourceOwnership,
       AsyncCommitIntentProgressWitness, CommitIntentProgressWitness,
       CommitRecoveryAuthority, ActiveLockedCommitIntent,
       RetainedCommitIntent, NodeHasDecision,
       AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch

THEOREM ResponsiveReplayContinuationPreservesCommitSourceAuxiliaryInvariant ==
  /\ AsyncStrongTypeInvariant
  /\ CommitSourceRetentionInvariant
  /\ ResponsiveReplayContinuationCommitSourceTransition
  => CommitSourceAuxiliaryInvariant'
PROOF
  <1>1. ASSUME AsyncStrongTypeInvariant,
              CommitSourceRetentionInvariant,
              ResponsiveReplayContinuationCommitSourceTransition
         PROVE CommitSourceAuxiliaryInvariant'
    <2>1. CASE DriveResponsiveReplayHead
      BY <1>1, <2>1,
         DriveResponsiveReplayPreservesCommitSourceAuxiliaryInvariant
    <2>2. CASE FinishResponsiveReplay
      BY <1>1, <2>2,
         FinishResponsiveReplayPreservesCommitSourceAuxiliaryInvariant
    <2>3. CASE RearmResponsiveRecovery
      BY <1>1, <2>3,
         RearmResponsiveRecoveryPreservesCommitSourceAuxiliaryInvariant
    <2> QED BY <1>1, <2>1, <2>2, <2>3
         DEF ResponsiveReplayContinuationCommitSourceTransition
  <1> QED BY <1>1

THEOREM AsyncNextPreservesCommitSourceRetention ==
  /\ AsyncStrongTypeInvariant
  /\ CommitSourceRetentionInvariant
  /\ AsyncNext
  => CommitSourceRetentionInvariant'
PROOF
  <1>1. ASSUME AsyncStrongTypeInvariant,
              CommitSourceRetentionInvariant,
              AsyncNext
         PROVE CommitSourceRetentionInvariant'
    <2>1. ResponsiveCommitIntentLockBound'
      BY <1>1, AsyncNextPreservesResponsiveCommitIntentLockBound
         DEF AsyncStrongTypeInvariant,
             CommitSourceRetentionInvariant
    <2>2. \/ UNCHANGED CommitSourceProjection
           \/ \E command: ExecuteSignVote(command)
           \/ \E lockRequest \in pendingLockCommit:
                PersistLockCommitSourceTransition(lockRequest)
           \/ \E installRequest \in pendingInstallTC:
                PersistInstallCommitSourceTransition(installRequest)
           \/ \E node \in ValidatorIds:
                ResponsiveCrashCommitSourceTransition(node)
           \/ ResponsiveRestartCommitSourceTransition
           \/ ResponsiveReplayCommitSourceTransition
           \/ ResponsiveReplayContinuationCommitSourceTransition
      BY <1>1, AsyncNextHasCommitSourceTransition
    <2>3. CASE UNCHANGED CommitSourceProjection
      BY <1>1, <2>3,
         CommitSourceProjectionFramePreservesAuxiliaryInvariant
         DEF CommitSourceRetentionInvariant
    <2>4. CASE \E command: ExecuteSignVote(command)
      <3>1. ASSUME NEW command, ExecuteSignVote(command)
             PROVE CommitSourceAuxiliaryInvariant'
        BY <1>1, <3>1,
           ExecuteSignVotePreservesCommitSourceAuxiliaryInvariant
      <3> QED BY <2>4, <3>1
    <2>5. CASE \E request \in pendingLockCommit:
                   PersistLockCommitSourceTransition(request)
      <3>1. ASSUME NEW request \in pendingLockCommit,
                    PersistLockCommitSourceTransition(request)
             PROVE CommitSourceAuxiliaryInvariant'
        BY <1>1, <3>1,
           PersistLockPreservesCommitSourceAuxiliaryInvariant
      <3> QED BY <2>5, <3>1
    <2>6. CASE \E request \in pendingInstallTC:
                   PersistInstallCommitSourceTransition(request)
      <3>1. ASSUME NEW request \in pendingInstallTC,
                    PersistInstallCommitSourceTransition(request)
             PROVE CommitSourceAuxiliaryInvariant'
        BY <1>1, <3>1,
           PersistInstallPreservesCommitSourceAuxiliaryInvariant
      <3> QED BY <2>6, <3>1
    <2>7. CASE \E node \in ValidatorIds:
                   ResponsiveCrashCommitSourceTransition(node)
      <3>1. ASSUME NEW node \in ValidatorIds,
                    ResponsiveCrashCommitSourceTransition(node)
             PROVE CommitSourceAuxiliaryInvariant'
        BY <1>1, <3>1,
           ResponsiveCrashPreservesCommitSourceAuxiliaryInvariant
      <3> QED BY <2>7, <3>1
    <2>8. CASE ResponsiveRestartCommitSourceTransition
      BY <1>1, <2>8,
         ResponsiveRestartPreservesCommitSourceAuxiliaryInvariant
    <2>9. CASE ResponsiveReplayCommitSourceTransition
      BY <1>1, <2>9,
         ResponsiveReplayPreservesCommitSourceAuxiliaryInvariant
    <2>10. CASE ResponsiveReplayContinuationCommitSourceTransition
      BY <1>1, <2>10,
         ResponsiveReplayContinuationPreservesCommitSourceAuxiliaryInvariant
    <2>11. CommitSourceAuxiliaryInvariant'
      BY <2>2, <2>3, <2>4, <2>5, <2>6, <2>7, <2>8, <2>9,
         <2>10
    <2> QED BY <2>1, <2>11 DEF CommitSourceRetentionInvariant,
                                CommitSourceAuxiliaryInvariant
  <1> QED BY <1>1

THEOREM CommitSourceRetentionProvidesDurableCommitWitness ==
  CommitSourceRetentionInvariant => AsyncDurableCommitProgressWitness
PROOF
  <1>1. ASSUME CommitSourceRetentionInvariant
         PROVE AsyncDurableCommitProgressWitness
    <2>1. ASSUME NEW node \in AsyncCurrentResponsiveVoters,
                  NEW vote \in commitIntents,
                  ActiveLockedCommitIntent(node, vote)
           PROVE AsyncCommitIntentProgressWitness(node, vote)
      <3>1. AsyncCommitIntentProgressWitness(node, vote)
        BY <1>1, <2>1
           DEF CommitSourceRetentionInvariant,
               ActiveCommitIntentSourceOwnership
      <3> QED BY <3>1
    <2> QED BY <2>1 DEF AsyncDurableCommitProgressWitness
  <1> QED BY <1>1

THEOREM AsyncAllVarsStutterPreservesCommitSourceRetention ==
  /\ CommitSourceRetentionInvariant
  /\ UNCHANGED AsyncAllVars
  => CommitSourceRetentionInvariant'
BY Isa
   DEF CommitSourceRetentionInvariant,
       ResponsiveCommitIntentLockBound,
       ResponsiveRetainedCommitControlSound,
       ActiveCommitIntentSourceOwnership,
       AsyncCommitIntentProgressWitness, CommitIntentProgressWitness,
       CommitRecoveryAuthority,
       ActiveLockedCommitIntent, RetainedCommitIntent,
       AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch,
       AsyncAllVars, AsyncSchedulerVars, vars

THEOREM AsyncBracketNextPreservesCommitSourceRetention ==
  /\ AsyncStrongTypeInvariant
  /\ CommitSourceRetentionInvariant
  /\ [AsyncNext]_AsyncAllVars
  => CommitSourceRetentionInvariant'
PROOF
  <1>1. ASSUME AsyncStrongTypeInvariant,
              CommitSourceRetentionInvariant,
              [AsyncNext]_AsyncAllVars
         PROVE CommitSourceRetentionInvariant'
    <2>1. CASE AsyncNext
      BY <1>1, <2>1, AsyncNextPreservesCommitSourceRetention
    <2>2. CASE UNCHANGED AsyncAllVars
      BY <1>1, <2>2,
         AsyncAllVarsStutterPreservesCommitSourceRetention
    <2> QED BY <1>1, <2>1, <2>2
  <1> QED BY <1>1

THEOREM CommitSourceRetentionInvariantObligation ==
  \A initialContext:
    AsyncSpecAt(initialContext) => []CommitSourceRetentionInvariant
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE AsyncSpecAt(initialContext)
                 => []CommitSourceRetentionInvariant
    <2> DEFINE Inductive ==
           /\ AsyncStrongTypeInvariant
           /\ CommitSourceRetentionInvariant
    <2>1. AsyncInitAt(initialContext) => Inductive
      BY AsyncInitEstablishesStrongTypeInvariant,
         AsyncInitEstablishesCommitSourceRetention
         DEF Inductive
    <2>2. Inductive /\ [AsyncNext]_AsyncAllVars => Inductive'
      BY AsyncBracketNextPreservesStrongTypeInvariant,
         AsyncBracketNextPreservesCommitSourceRetention
         DEF Inductive
    <2>3. AsyncSpecAt(initialContext) => []Inductive
      BY <2>1, <2>2, PTL DEF AsyncSpecAt
    <2>4. Inductive => CommitSourceRetentionInvariant
      BY DEF Inductive
    <2> QED BY <2>3, <2>4, PTL
  <1> QED BY <1>1

THEOREM DurableCommitProgressWitnessObligation ==
  \A initialContext:
    AsyncSpecAt(initialContext) => []AsyncDurableCommitProgressWitness
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE AsyncSpecAt(initialContext)
                 => []AsyncDurableCommitProgressWitness
    <2>1. AsyncSpecAt(initialContext)
             => []CommitSourceRetentionInvariant
      BY CommitSourceRetentionInvariantObligation
    <2>2. CommitSourceRetentionInvariant
             => AsyncDurableCommitProgressWitness
      BY CommitSourceRetentionProvidesDurableCommitWitness
    <2> QED BY <2>1, <2>2, PTL
  <1> QED BY <1>1

THEOREM AsyncCrashAwareProgressWitnessComponentsObligation ==
  \A initialContext:
    AsyncSpecAt(initialContext)
      => [](/\ AsyncDurableCommitProgressWitness
            /\ ProtectedDeferredProgressInvariant)
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE AsyncSpecAt(initialContext)
                 => [](/\ AsyncDurableCommitProgressWitness
                       /\ ProtectedDeferredProgressInvariant)
    <2>1. AsyncSpecAt(initialContext)
             => []AsyncDurableCommitProgressWitness
      BY DurableCommitProgressWitnessObligation
    <2>2. AsyncSpecAt(initialContext)
             => []ProtectedDeferredProgressInvariant
      BY ProtectedDeferredProgressInvariantObligation
    <2> QED BY <2>1, <2>2, PTL
  <1> QED BY <1>1

(***************************************************************************
Decision is a terminal frontier for timeout control at the current height.
The two reducer constructors below are structurally disabled once the node
has a durable Decision.  Authenticated timeout traffic may still be consumed,
but its atomic receipt turn cannot create an InstallTC WAL authority and
BeginInstallTC cannot be scheduled.  The state invariant also excludes latent
pending timeout writes, pending TC installs, and timeout-signature owners.  Its
pending-Decision conjunct is the historical strengthening required to prove
that PersistDecision cannot expose such an owner after installing Decision.

The direct facts are deliberately separated from the temporal obligation.
They make the new guards and consume-only branches reviewable without
claiming that the complete Async action decomposition has already been
machine checked against every scheduler successor.
***************************************************************************)

PendingTimeoutExcludesDecision ==
  \A request \in pendingTimeout:
    NoDecisionForNode(request.node)

PendingInstallExcludesDecision ==
  \A request \in pendingInstallTC:
    NoDecisionForNode(request.node)

TimeoutSigningExcludesDecision ==
  \A request \in signTimeouts:
    NoDecisionForNode(request.node)

PendingDecisionExcludesTimeoutWork ==
  \A request \in pendingDecision:
    /\ request.node \notin RequestNodeSet(pendingTimeout)
    /\ request.node \notin RequestNodeSet(pendingInstallTC)
    /\ request.node \notin RequestNodeSet(signTimeouts)

DecisionTimeoutFrontierInvariant ==
  /\ PendingTimeoutExcludesDecision
  /\ PendingInstallExcludesDecision
  /\ TimeoutSigningExcludesDecision
  /\ PendingDecisionExcludesTimeoutWork

PostDecisionTimeoutControlExcluded ==
  \A node:
    ~NoDecisionForNode(node)
      => /\ ~BeginTimeout(node)
         /\ \A tc: ~BeginInstallTC(node, tc)

PostDecisionTimeoutTrafficConsumeOnly ==
  /\ \A envelope:
       /\ ~NoDecisionForNode(envelope.recipient)
       /\ DeliverTimeout(envelope)
       => /\ timeoutNetwork' = timeoutNetwork \ {envelope}
          /\ receivedTimeoutVotes' = receivedTimeoutVotes
  /\ \A envelope:
       /\ ~NoDecisionForNode(envelope.recipient)
       /\ DeliverTC(envelope)
       => /\ tcNetwork' = tcNetwork \ {envelope}
          /\ receivedTCs' = receivedTCs

PostDecisionTimeoutCausalSuccessorsExcluded ==
  \A command \in AsyncCandidateSet:
    ~NoDecisionForNode(command.node)
      => /\ (command.kind = "DeliverTimeout"
               => CommandSuccessors(command) = <<>>)
         /\ (command.kind = "DeliverTC"
               => CommandSuccessors(command) = <<>>)

PostDecisionTimeoutExclusionProperty(specification) ==
  /\ specification => []DecisionTimeoutFrontierInvariant
  /\ specification
       => [][PostDecisionTimeoutControlExcluded]_AsyncAllVars
  /\ specification
       => [][PostDecisionTimeoutTrafficConsumeOnly]_AsyncAllVars
  /\ specification => []PostDecisionTimeoutCausalSuccessorsExcluded

THEOREM PostDecisionTimeoutControlGuardsAreStructural ==
  PostDecisionTimeoutControlExcluded
BY Isa
   DEF PostDecisionTimeoutControlExcluded, BeginTimeout, BeginInstallTC

THEOREM AsyncInitEstablishesDecisionTimeoutFrontier ==
  \A initialContext:
    AsyncInitAt(initialContext) => DecisionTimeoutFrontierInvariant
BY Isa
   DEF AsyncInitAt, AsyncBaseInitAt, InitAt,
       DecisionTimeoutFrontierInvariant,
       PendingTimeoutExcludesDecision,
       PendingInstallExcludesDecision,
       TimeoutSigningExcludesDecision,
       PendingDecisionExcludesTimeoutWork, RequestNodeSet

THEOREM PostDecisionTimeoutDeliveryIsConsumeOnly ==
  PostDecisionTimeoutTrafficConsumeOnly
BY Isa
   DEF PostDecisionTimeoutTrafficConsumeOnly, DeliverTimeout, DeliverTC

THEOREM PostDecisionTimeoutCausalSuccessorsAreEmpty ==
  PostDecisionTimeoutCausalSuccessorsExcluded
BY DEF PostDecisionTimeoutCausalSuccessorsExcluded, CommandSuccessors

DecisionTimeoutFrontierVars ==
  <<context, decisions, pendingTimeout, pendingInstallTC,
    pendingDecision, signTimeouts>>

DecisionTimeoutFrontierStutteringStep ==
  \/ SetGST
  \/ \E node \in ValidatorIds, subject \in Subjects:
       AssembleLocalBody(node, subject)
  \/ \E node \in ValidatorIds, subject \in Subjects:
       BeginLocalProposal(node, subject)
  \/ \E request \in pendingProposal: PersistProposal(request)
  \/ \E request \in signProposals: CompleteProposalSignature(request)
  \/ \E signer \in ValidatorIds, roundView \in Views,
       subject \in Subjects,
       timeoutCertificate \in TimeoutCertificateOptionSet,
       highestPrepare \in PrepareQcOptionSet:
       ByzantineBroadcastProposal(signer, roundView, subject,
                                  timeoutCertificate, highestPrepare)
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
  \/ \E envelope \in QcEnvelopeSet:
       ImportAuthenticatedCommitCertificate(envelope)
  \/ \E envelope \in qcNetwork: DeliverQC(envelope)
  \/ \E node \in ValidatorIds, qc \in ReceivedQcValues:
       BeginObservePrepare(node, qc)
  \/ \E request \in pendingObservePrepare: PersistObservePrepare(request)
  \/ \E node \in ValidatorIds, qc \in LockCommitQcValues:
       BeginLockCommit(node, qc)
  \/ \E request \in pendingLockCommit: PersistLockCommit(request)
  \/ \E signer \in ValidatorIds, roundView \in Views,
       highestPrepare \in PrepareQcOptionSet:
       ByzantineBroadcastTimeout(signer, roundView, highestPrepare)
  \/ \E envelope \in timeoutNetwork: DeliverTimeout(envelope)
  \/ \E envelope \in tcNetwork: DeliverTC(envelope)
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
  \/ \E envelope \in proposalNetwork: DropProposal(envelope)

THEOREM DecisionTimeoutFrontierStutteringStepIsStutter ==
  DecisionTimeoutFrontierStutteringStep
    => UNCHANGED DecisionTimeoutFrontierVars
BY SMTT(120), IsaT(120)
   DEF DecisionTimeoutFrontierStutteringStep,
       DecisionTimeoutFrontierVars,
       SetGST, AssembleLocalBody, BeginLocalProposal, PersistProposal,
       CompleteProposalSignature, ByzantineBroadcastProposal,
       DeliverProposal, FetchBody, RebindRetainedBody, StoreBody,
       ValidateBody, ValidateDecidedBody, ValidateLockedBody, RejectBody,
       BeginPrepare,
       PersistPrepare, CompleteVoteSignature, ByzantineBroadcastVote,
       DeliverVote, FormPrepareQC,
       ImportAuthenticatedCommitCertificate, DeliverQC,
       BeginObservePrepare, PersistObservePrepare, BeginLockCommit,
       PersistLockCommit, ByzantineBroadcastTimeout, DeliverTimeout,
       DeliverTC, FetchCertifiedBody, ApplyDecision, Restart,
       ResumeProposal, ResumeVote, DropProposal

THEOREM DecisionTimeoutFrontierStutterPreservesInvariant ==
  /\ DecisionTimeoutFrontierInvariant
  /\ UNCHANGED DecisionTimeoutFrontierVars
  => DecisionTimeoutFrontierInvariant'
PROOF
  <1>1. ASSUME DecisionTimeoutFrontierInvariant,
              UNCHANGED DecisionTimeoutFrontierVars
         PROVE DecisionTimeoutFrontierInvariant'
    <2>1. /\ context' = context
           /\ decisions' = decisions
           /\ pendingTimeout' = pendingTimeout
           /\ pendingInstallTC' = pendingInstallTC
           /\ pendingDecision' = pendingDecision
           /\ signTimeouts' = signTimeouts
      BY <1>1, Isa DEF DecisionTimeoutFrontierVars
    <2> QED BY <1>1, <2>1, Isa
         DEF DecisionTimeoutFrontierInvariant,
             PendingTimeoutExcludesDecision,
             PendingInstallExcludesDecision,
             TimeoutSigningExcludesDecision,
             PendingDecisionExcludesTimeoutWork,
             NoDecisionForNode, RequestNodeSet
  <1> QED BY <1>1

THEOREM FormCommitQcPreservesDecisionTimeoutFrontier ==
  \A node, roundView, subject:
    /\ DecisionTimeoutFrontierInvariant
    /\ FormCommitQC(node, roundView, subject)
    => DecisionTimeoutFrontierInvariant'
BY SMTT(120), IsaT(120)
   DEF DecisionTimeoutFrontierInvariant,
       PendingTimeoutExcludesDecision,
       PendingInstallExcludesDecision,
       TimeoutSigningExcludesDecision,
       PendingDecisionExcludesTimeoutWork,
       NoDecisionForNode, RequestNodeSet, NodeIdle, PendingNodes,
       SigningNodes, FormCommitQC, DecisionWal

THEOREM BeginDecisionPreservesDecisionTimeoutFrontier ==
  \A node, qc:
    /\ DecisionTimeoutFrontierInvariant
    /\ BeginDecision(node, qc)
    => DecisionTimeoutFrontierInvariant'
BY SMTT(120), IsaT(120)
   DEF DecisionTimeoutFrontierInvariant,
       PendingTimeoutExcludesDecision,
       PendingInstallExcludesDecision,
       TimeoutSigningExcludesDecision,
       PendingDecisionExcludesTimeoutWork,
       NoDecisionForNode, RequestNodeSet, NodeIdle, PendingNodes,
       SigningNodes, BeginDecision, DecisionWal

THEOREM PersistDecisionPreservesDecisionTimeoutFrontier ==
  \A request:
    /\ DecisionTimeoutFrontierInvariant
    /\ PersistDecision(request)
    => DecisionTimeoutFrontierInvariant'
BY SMTT(120), IsaT(120)
   DEF DecisionTimeoutFrontierInvariant,
       PendingTimeoutExcludesDecision,
       PendingInstallExcludesDecision,
       TimeoutSigningExcludesDecision,
       PendingDecisionExcludesTimeoutWork,
       NoDecisionForNode, RequestNodeSet, PersistDecision

THEOREM BeginTimeoutPreservesDecisionTimeoutFrontier ==
  \A node:
    /\ DecisionTimeoutFrontierInvariant
    /\ BeginTimeout(node)
    => DecisionTimeoutFrontierInvariant'
BY SMTT(120), IsaT(120)
   DEF DecisionTimeoutFrontierInvariant,
       PendingTimeoutExcludesDecision,
       PendingInstallExcludesDecision,
       TimeoutSigningExcludesDecision,
       PendingDecisionExcludesTimeoutWork,
       NoDecisionForNode, RequestNodeSet, NodeIdle, PendingNodes,
       SigningNodes, BeginTimeout, TimeoutRequestFor, TimeoutWal

THEOREM PersistTimeoutPreservesDecisionTimeoutFrontier ==
  \A request:
    /\ DecisionTimeoutFrontierInvariant
    /\ PersistTimeout(request)
    => DecisionTimeoutFrontierInvariant'
BY SMTT(120), IsaT(120)
   DEF DecisionTimeoutFrontierInvariant,
       PendingTimeoutExcludesDecision,
       PendingInstallExcludesDecision,
       TimeoutSigningExcludesDecision,
       PendingDecisionExcludesTimeoutWork,
       NoDecisionForNode, RequestNodeSet, PersistTimeout, TimeoutSign

THEOREM CompleteTimeoutSignaturePreservesDecisionTimeoutFrontier ==
  \A request:
    /\ DecisionTimeoutFrontierInvariant
    /\ CompleteTimeoutSignature(request)
    => DecisionTimeoutFrontierInvariant'
BY SMTT(120), IsaT(120)
   DEF DecisionTimeoutFrontierInvariant,
       PendingTimeoutExcludesDecision,
       PendingInstallExcludesDecision,
       TimeoutSigningExcludesDecision,
       PendingDecisionExcludesTimeoutWork,
       NoDecisionForNode, RequestNodeSet,
       CompleteTimeoutSignature

THEOREM DeliverTimeoutPreservesDecisionTimeoutFrontier ==
  \A envelope:
    /\ DecisionTimeoutFrontierInvariant
    /\ DeliverTimeout(envelope)
    => DecisionTimeoutFrontierInvariant'
BY SMTT(120), IsaT(120)
   DEF DecisionTimeoutFrontierInvariant,
       PendingTimeoutExcludesDecision,
       PendingInstallExcludesDecision,
       TimeoutSigningExcludesDecision,
       PendingDecisionExcludesTimeoutWork,
       NoDecisionForNode, RequestNodeSet, DeliverTimeout

THEOREM BeginInstallTcPreservesDecisionTimeoutFrontier ==
  \A node, tc:
    /\ DecisionTimeoutFrontierInvariant
    /\ BeginInstallTC(node, tc)
    => DecisionTimeoutFrontierInvariant'
BY SMTT(120), IsaT(120)
   DEF DecisionTimeoutFrontierInvariant,
       PendingTimeoutExcludesDecision,
       PendingInstallExcludesDecision,
       TimeoutSigningExcludesDecision,
       PendingDecisionExcludesTimeoutWork,
       NoDecisionForNode, RequestNodeSet, NodeIdle, PendingNodes,
       SigningNodes, BeginInstallTC, InstallTcWal

THEOREM PersistInstallTcPreservesDecisionTimeoutFrontier ==
  \A request:
    /\ DecisionTimeoutFrontierInvariant
    /\ PersistInstallTC(request)
    => DecisionTimeoutFrontierInvariant'
BY SMTT(120), IsaT(120)
   DEF DecisionTimeoutFrontierInvariant,
       PendingTimeoutExcludesDecision,
       PendingInstallExcludesDecision,
       TimeoutSigningExcludesDecision,
       PendingDecisionExcludesTimeoutWork,
       NoDecisionForNode, RequestNodeSet, PersistInstallTC

THEOREM CrashPreservesDecisionTimeoutFrontier ==
  \A node:
    /\ DecisionTimeoutFrontierInvariant
    /\ Crash(node)
    => DecisionTimeoutFrontierInvariant'
BY SMTT(120), IsaT(120)
   DEF DecisionTimeoutFrontierInvariant,
       PendingTimeoutExcludesDecision,
       PendingInstallExcludesDecision,
       TimeoutSigningExcludesDecision,
       PendingDecisionExcludesTimeoutWork,
       NoDecisionForNode, RequestNodeSet, Crash

THEOREM ResumeTimeoutPreservesDecisionTimeoutFrontier ==
  \A node, vote:
    /\ DecisionTimeoutFrontierInvariant
    /\ ResumeTimeout(node, vote)
    => DecisionTimeoutFrontierInvariant'
BY SMTT(120), IsaT(120)
   DEF DecisionTimeoutFrontierInvariant,
       PendingTimeoutExcludesDecision,
       PendingInstallExcludesDecision,
       TimeoutSigningExcludesDecision,
       PendingDecisionExcludesTimeoutWork,
       NoDecisionForNode, RequestNodeSet, NodeIdle, PendingNodes,
       SigningNodes, ResumeTimeout, TimeoutSign

THEOREM CoreNextPreservesDecisionTimeoutFrontier ==
  /\ DecisionTimeoutFrontierInvariant
  /\ Next
  => DecisionTimeoutFrontierInvariant'
PROOF
  <1>1. ASSUME DecisionTimeoutFrontierInvariant, Next
         PROVE DecisionTimeoutFrontierInvariant'
    <2>1. CASE DecisionTimeoutFrontierStutteringStep
      <3>1. UNCHANGED DecisionTimeoutFrontierVars
        BY <2>1, DecisionTimeoutFrontierStutteringStepIsStutter
      <3> QED BY <1>1, <3>1,
           DecisionTimeoutFrontierStutterPreservesInvariant
    <2>2. CASE \E node \in ValidatorIds, roundView \in Views,
                    subject \in Subjects:
                    FormCommitQC(node, roundView, subject)
      BY <1>1, <2>2, FormCommitQcPreservesDecisionTimeoutFrontier
    <2>3. CASE \E node \in ValidatorIds, qc \in ReceivedQcValues:
                    BeginDecision(node, qc)
      BY <1>1, <2>3, BeginDecisionPreservesDecisionTimeoutFrontier
    <2>4. CASE \E request \in pendingDecision: PersistDecision(request)
      BY <1>1, <2>4, PersistDecisionPreservesDecisionTimeoutFrontier
    <2>5. CASE \E node \in ValidatorIds: BeginTimeout(node)
      BY <1>1, <2>5, BeginTimeoutPreservesDecisionTimeoutFrontier
    <2>6. CASE \E request \in pendingTimeout: PersistTimeout(request)
      BY <1>1, <2>6, PersistTimeoutPreservesDecisionTimeoutFrontier
    <2>7. CASE \E request \in signTimeouts:
                    CompleteTimeoutSignature(request)
      BY <1>1, <2>7,
         CompleteTimeoutSignaturePreservesDecisionTimeoutFrontier
    <2>8. CASE \E envelope \in timeoutNetwork: DeliverTimeout(envelope)
      BY <1>1, <2>8, DeliverTimeoutPreservesDecisionTimeoutFrontier
    <2>9. CASE \E node \in ValidatorIds, tc \in ReceivedTcValues:
                    BeginInstallTC(node, tc)
      BY <1>1, <2>9, BeginInstallTcPreservesDecisionTimeoutFrontier
    <2>10. CASE \E request \in pendingInstallTC:
                     PersistInstallTC(request)
      BY <1>1, <2>10, PersistInstallTcPreservesDecisionTimeoutFrontier
    <2>11. CASE \E node \in ValidatorIds: Crash(node)
      BY <1>1, <2>11, CrashPreservesDecisionTimeoutFrontier
    <2>12. CASE \E node \in ValidatorIds, vote \in timeoutIntents:
                     ResumeTimeout(node, vote)
      BY <1>1, <2>12, ResumeTimeoutPreservesDecisionTimeoutFrontier
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4, <2>5, <2>6,
                <2>7, <2>8, <2>9, <2>10, <2>11, <2>12
         DEF Next, DecisionTimeoutFrontierStutteringStep
  <1> QED BY <1>1

THEOREM CoreStutterPreservesDecisionTimeoutFrontier ==
  /\ DecisionTimeoutFrontierInvariant
  /\ UNCHANGED vars
  => DecisionTimeoutFrontierInvariant'
BY CoreNextPreservesDecisionTimeoutFrontier, Isa
   DEF DecisionTimeoutFrontierInvariant,
       PendingTimeoutExcludesDecision,
       PendingInstallExcludesDecision,
       TimeoutSigningExcludesDecision,
       PendingDecisionExcludesTimeoutWork,
       NoDecisionForNode, RequestNodeSet, vars

THEOREM CoreBracketPreservesDecisionTimeoutFrontier ==
  /\ DecisionTimeoutFrontierInvariant
  /\ [Next]_vars
  => DecisionTimeoutFrontierInvariant'
BY CoreNextPreservesDecisionTimeoutFrontier,
   CoreStutterPreservesDecisionTimeoutFrontier, Isa

THEOREM AsyncBracketPreservesDecisionTimeoutFrontier ==
  /\ DecisionTimeoutFrontierInvariant
  /\ [AsyncNext]_AsyncAllVars
  => DecisionTimeoutFrontierInvariant'
PROOF
  <1>1. ASSUME DecisionTimeoutFrontierInvariant,
              [AsyncNext]_AsyncAllVars
         PROVE DecisionTimeoutFrontierInvariant'
    <2>1. CASE AsyncNext
      <3>1. [Next]_vars
        BY <2>1 DEF AsyncNext
      <3> QED BY <1>1, <3>1,
           CoreBracketPreservesDecisionTimeoutFrontier
    <2>2. CASE UNCHANGED AsyncAllVars
      <3>1. UNCHANGED vars
        BY <2>2 DEF AsyncAllVars, AsyncSchedulerVars
      <3> QED BY <1>1, <3>1,
           CoreStutterPreservesDecisionTimeoutFrontier
    <2> QED BY <1>1, <2>1, <2>2
  <1> QED BY <1>1

THEOREM DecisionTimeoutFrontierInvariantFromAsyncSpec ==
  \A initialContext:
    AsyncSpecAt(initialContext) => []DecisionTimeoutFrontierInvariant
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE AsyncSpecAt(initialContext)
                 => []DecisionTimeoutFrontierInvariant
    <2>1. AsyncInitAt(initialContext)
            => DecisionTimeoutFrontierInvariant
      BY AsyncInitEstablishesDecisionTimeoutFrontier
    <2>2. /\ DecisionTimeoutFrontierInvariant
           /\ [AsyncNext]_AsyncAllVars
          => DecisionTimeoutFrontierInvariant'
      BY AsyncBracketPreservesDecisionTimeoutFrontier
    <2> QED BY <2>1, <2>2, PTL DEF AsyncSpecAt
  <1> QED BY <1>1

(***************************************************************************
The body-state-classified recovery frontier emitted after decision persistence
is owned by the trusted completion pipeline.  It retains the causal command's
exact identity, so the stage predicate recognizes the scheduled occurrence
rather than reconstructing a weaker NoAsyncItem value.  The occurrence counts
only while its consumer context, view, and generation are current.  The stage
invariant below records the exact executable owner at each durable-body
boundary; in particular, it does not treat Apply as sufficient recovery
ownership before the body and validation witnesses exist.
***************************************************************************)

DecisionBody(node, qc) ==
  BodyRecord(node, qc.context, qc.view, qc.subject)

DecisionValidationHeld(node, qc) ==
  \E validation \in validatedBodies:
    /\ validation.node = node
    /\ validation.context = qc.context
    /\ validation.view = qc.view
    /\ validation.subject = qc.subject

(***************************************************************************
The real Decision continuation may carry the Decision QC, a vote, or an
authenticated response item as evidence.  Ownership is therefore existential
over the full candidate record; reconstructing a NoAsyncItem candidate loses
that identity and incorrectly reports an empty recovery stage.
***************************************************************************)
DecisionPipelineKindOwned(node, qc, kind) ==
  \E candidate \in AsyncCandidateSet:
    /\ candidate.kind = kind
    /\ DecisionPipelineCandidate(node, qc, candidate)

DecisionCandidateOwned(node, qc, kind) ==
  DecisionPipelineKindOwned(node, qc, kind)

DecisionFetchBodyOwned(node, qc) ==
  DecisionPipelineKindOwned(node, qc, "FetchBody")

DecisionRecoveryCertificate(node, qc, recoveryQc) ==
  /\ recoveryQc.context = qc.context
  /\ recoveryQc.view = qc.view
  /\ recoveryQc.subject = qc.subject
  /\ recoveryQc.phase = "Commit"
  /\ \E decision \in decisions:
       /\ decision.node = node
       /\ decision.qc = recoveryQc

DecisionCertifiedFetchOwned(node, qc) ==
  \E item \in AsyncNetworkItems, recoveryQc \in QcRecordSet:
    /\ DecisionRecoveryCertificate(node, qc, recoveryQc)
    /\ item.kind = "CertifiedResponse"
    /\ CertifiedResponseAuthenticatedOccurrence(item)
    /\ item.envelope.recipient = node
    /\ item.envelope.height = qc.context.height
    /\ item.envelope.view = qc.view
    /\ item.envelope.subject = qc.subject
    /\ item.envelope.requestHash =
         AsyncCertifiedRequestHashOf(node, recoveryQc, 0)
    /\ item.envelope.signatureOwner = item.envelope.responder
    /\ item.envelope.responder \in AsyncArchiveServerIds
    /\ CertifiedResponseCandidate(item) \in AsyncCandidateSet
    /\ CandidateScheduled(CertifiedResponseCandidate(item))

DecisionCertifiedRequestActive(node, qc) ==
  \E request \in asyncActiveRequests, recoveryQc \in QcRecordSet:
    /\ DecisionRecoveryCertificate(node, qc, recoveryQc)
    /\ request.kind = "CertifiedRequest"
    /\ request.source = node
    /\ request.envelope.requester = node
    /\ request.envelope.certificate = recoveryQc
    /\ request.envelope.signatureNonce = 0
    /\ request.envelope.recipient
         \in CertifiedArchiveRoutes(node, recoveryQc)
    /\ request.envelope.height = qc.context.height
    /\ request.envelope.view = qc.view
    /\ request.envelope.subject = qc.subject

DecisionRecoveryStage(node, qc) ==
  \/ NodeHasApplication(node)
  \/ /\ ~BodyHeldBy(durableBodies, node, qc.context,
                     qc.view, qc.subject)
     /\ \/ DecisionFetchBodyOwned(node, qc)
        \/ DecisionCertifiedRequestActive(node, qc)
        \/ DecisionPipelineKindOwned(
             node, qc, "RequestCertifiedBody")
        \/ DecisionCertifiedFetchOwned(node, qc)
        \/ /\ DecisionBody(node, qc) \in availableBodies
           /\ DecisionPipelineKindOwned(node, qc, "StoreBody")
  \/ /\ BodyHeldBy(durableBodies, node, qc.context,
                    qc.view, qc.subject)
     /\ ~DecisionValidationHeld(node, qc)
     /\ DecisionPipelineKindOwned(node, qc, "ValidateBody")
  \/ /\ BodyHeldBy(durableBodies, node, qc.context,
                    qc.view, qc.subject)
     /\ DecisionValidationHeld(node, qc)
     /\ DecisionPipelineKindOwned(node, qc, "Apply")

(***************************************************************************
A responsive crash removes every volatile Decision-pipeline owner.  The
durable Decision itself therefore authorizes exactly one recovery lifecycle,
bound to the recovering node and generation, until replay reconstructs a
current-consumer FetchBody owner.  The authority applies only to an unapplied
Decision in the current context; an applied Decision remains terminal through
NodeHasApplication instead.

Decision uniqueness is stated explicitly because RestartDecision chooses one
durable record.  Without uniqueness by node/context, a single replay frontier
would not be an exact witness for every durable Decision QC.
***************************************************************************)

DecisionRecoveryAuthority(node, qc) ==
  /\ DurableDecisionRecoveryAuthority(node, qc)
  /\ DurableDecisionRecoveryExecutorCurrent(node)

AsyncDecisionRecoveryStage(node, qc) ==
  \/ DecisionRecoveryStage(node, qc)
  \/ DecisionRecoveryAuthority(node, qc)

AsyncDecisionCompletionWitness(node, qc) ==
  \/ DecisionCompletionWitness(node, qc)
  \/ DecisionRecoveryAuthority(node, qc)

AsyncDurableDecisionProgressWitness ==
  \A decision \in decisions:
    (decision.node \in AsyncCurrentResponsiveVoters
      /\ decision.qc.context = context)
      => AsyncDecisionCompletionWitness(decision.node, decision.qc)

DecisionSourceRetentionInvariant ==
  \A decision \in decisions:
    (decision.node \in AsyncCurrentResponsiveVoters
      /\ decision.qc.context = context)
      => DecisionRecoveryStage(decision.node, decision.qc)

AsyncDecisionSourceRetentionInvariant ==
  /\ DecisionsUniqueByNodeContext
  /\ \A decision \in decisions:
       (decision.node \in AsyncCurrentResponsiveVoters
         /\ decision.qc.context = context)
         => AsyncDecisionRecoveryStage(decision.node, decision.qc)

(***************************************************************************
The generic progress witness above intentionally keeps its all-stage,
generation-scoped authority.  The imported durable-Decision proof owns the
release-facing crash/restart/replay theorem: its Commit-only authority and
generation-free logical registration are separate from the current-generation
executor candidate.
***************************************************************************)

(***************************************************************************
The release-facing progress witness is recovery-aware for both durable
sources.  Commit authority owns WAL signature reconstruction; Decision
authority owns the exact crash/restart generation until replay reconstructs
a current-consumer FetchBody frontier.  Neither authority stands in for the
still-separate historical-lock preservation obligation.
***************************************************************************)

AsyncProgressWitnessInvariant ==
  /\ AsyncDurableCommitProgressWitness
  /\ HistoricalLockedCommitRecoveryProgress
  /\ DecisionsUniqueByNodeContext
  /\ AsyncDurableDecisionProgressWitness
  /\ ProtectedDeferredProgressInvariant

AsyncProgressWitnessProperty(specification) ==
  specification => []AsyncProgressWitnessInvariant

(***************************************************************************
The release-facing progress witness also owns the source-neutral historical
locked-body pipeline.  Keeping this conjunction under the one reviewed ledger
symbol prevents the pipeline from becoming an unledgered proofless theorem
while retaining its independent model predicate and proof obligation.
***************************************************************************)
AsyncProgressWitnessAndHistoricalRecoveryProperty(specification) ==
  /\ AsyncProgressWitnessProperty(specification)
  /\ HistoricalLockedBodyRecoveryProperty(specification)

THEOREM PersistDecisionRecoveryUsesBodyStateCompletion ==
  \A command:
    /\ command.kind = "PersistDecision"
    /\ PersistDecisionRequests(command) # {}
      => LET request == PersistDecisionRequest(command)
             qc == request.qc
             successor == PersistDecisionRecoverySuccessor(command)
         IN /\ CommandSuccessors(command) = <<successor>>
         /\ Len(CommandSuccessors(command)) = 1
         /\ successor.kind = PersistDecisionRecoveryKind(command)
         /\ successor.class = "Completion"
         /\ successor.item = NoAsyncItem
         /\ successor.evidence = qc
         /\ (/\ BodyHeldBy(durableBodies, request.node, qc.context,
                           qc.view, qc.subject)
               /\ PersistDecisionValidationHeld(command)
               => successor.kind = "Apply")
         /\ (/\ BodyHeldBy(durableBodies, request.node, qc.context,
                           qc.view, qc.subject)
               /\ ~PersistDecisionValidationHeld(command)
               => successor.kind = "ValidateBody")
         /\ (/\ ~BodyHeldBy(durableBodies, request.node, qc.context,
                            qc.view, qc.subject)
               /\ PersistDecisionBody(command) \in availableBodies
               => successor.kind = "StoreBody")
         /\ (/\ ~BodyHeldBy(durableBodies, request.node, qc.context,
                            qc.view, qc.subject)
               /\ PersistDecisionBody(command) \notin availableBodies
               => successor.kind = "FetchBody")
         /\ (CandidateConsumerCurrent(command)
               => CandidateConsumerCurrent(successor))
BY DEF CommandSuccessors, PersistDecisionRecoverySuccessor,
       PersistDecisionRecoveryKind, PersistDecisionBody,
       PersistDecisionValidationHeld, PersistDecisionRequest,
       AsyncCandidateCausalSuccessorWithIdentityAndOrigin,
       AsyncCandidateSuccessorProposalRound,
       AsyncCandidateWithIdentityAndOrigin,
       CandidateConsumerCurrent, PersistDecisionRequests

THEOREM CompletionDeferralRetainsCandidate ==
  \A candidate \in AsyncCandidateSet:
    /\ AsyncTypeInvariant
    /\ candidate.class = "Completion"
    /\ DeferCommand(candidate)
    => CandidateScheduled(candidate)'
PROOF
  <1>1. ASSUME NEW candidate \in AsyncCandidateSet,
                AsyncTypeInvariant,
                candidate.class = "Completion",
                DeferCommand(candidate)
         PROVE CandidateScheduled(candidate)'
    <2> DEFINE Queue ==
          asyncDeferredCompletionQueues[candidate.node]
    <2>1. candidate.node \in ValidatorIds
      BY <1>1 DEF AsyncCandidateSet
    <2>2. candidate.node \in DOMAIN asyncDeferredCompletionQueues
      BY <1>1, <2>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncDeferredTypeInvariant,
             AsyncDeferredTopologyTypeInvariant
    <2>3. Queue \in Seq(Range(Queue))
      BY <1>1, <2>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncDeferredTypeInvariant,
             AsyncDeferredContentTypeInvariant,
             AsyncCompletionSequenceTyped, Queue
    <2>4. asyncDeferredCompletionQueues' =
             [asyncDeferredCompletionQueues EXCEPT
                ![candidate.node] =
                  IF candidate \in SequenceSet(Queue)
                    THEN Queue
                    ELSE Append(Queue, candidate)]
      BY <1>1
         DEF DeferCommand, Queue
    <2>5. asyncDeferredCompletionQueues'[candidate.node] =
             IF candidate \in SequenceSet(Queue)
               THEN Queue
               ELSE Append(Queue, candidate)
      BY <2>2, <2>4, FunctionalReplaceUpdateAtKey
    <2>6. candidate \in
             SequenceSet(
               asyncDeferredCompletionQueues'[candidate.node])
      <3>1. CASE candidate \in SequenceSet(Queue)
        <4>1. asyncDeferredCompletionQueues'[candidate.node] = Queue
          BY <2>5, <3>1
        <4> QED BY <3>1, <4>1
      <3>2. CASE candidate \notin SequenceSet(Queue)
        <4>1. asyncDeferredCompletionQueues'[candidate.node] =
                 Append(Queue, candidate)
          BY <2>5, <3>2
        <4>2. SequenceSet(Append(Queue, candidate)) =
                 SequenceSet(Queue) \cup {candidate}
          BY <2>3, SequenceSetAfterAppend
        <4> QED BY <4>1, <4>2
      <3> QED BY <3>1, <3>2
    <2>7. candidate \in DeferredCandidates'
      BY <2>1, <2>6 DEF DeferredCandidates
    <2> QED BY <2>7 DEF CandidateScheduled
  <1> QED BY <1>1

THEOREM DecisionRecoveryCertificateHasRemoteBodySource ==
  \A node \in ValidatorIds, qc, recoveryQc \in QcRecordSet:
    /\ StrongInductiveInvariant
    /\ DecisionRecoveryCertificate(node, qc, recoveryQc)
    /\ ~BodyHeldBy(durableBodies, node, qc.context,
                    qc.view, qc.subject)
    => \E source \in recoveryQc.signers \ {node}:
         BodyHeldBy(durableBodies, source, qc.context,
                    qc.view, qc.subject)
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                NEW qc \in QcRecordSet,
                NEW recoveryQc \in QcRecordSet,
                StrongInductiveInvariant,
                DecisionRecoveryCertificate(node, qc, recoveryQc),
                ~BodyHeldBy(durableBodies, node, qc.context,
                             qc.view, qc.subject)
         PROVE \E source \in recoveryQc.signers \ {node}:
                 BodyHeldBy(durableBodies, source, qc.context,
                            qc.view, qc.subject)
    <2>1. recoveryQc \in commitQCs
      BY <1>1, SMT
         DEF StrongInductiveInvariant, Safety, DecisionAgreement,
             DecisionRecoveryCertificate
    <2>2. CertificateValidityAndAvailabilityInvariant
      BY <1>1,
         StrongInvariantImpliesCertificateValidityAndAvailability
    <2>3. CertificateValidityAndAvailability(
             recoveryQc, durableBodies, ValidSubjects)
      BY <2>1, <2>2
         DEF CertificateValidityAndAvailabilityInvariant
    <2> QED BY <1>1, <2>3, SMTT(30)
         DEF CertificateValidityAndAvailability,
             DecisionRecoveryCertificate
  <1> QED BY <1>1

THEOREM DecisionRecoveryCertificateHasResponsiveRemoteBodySource ==
  \A node \in ValidatorIds,
     qc \in QcRecordSet,
     recoveryQc \in QcRecordSet:
    /\ StrongInductiveInvariant
    /\ DecisionRecoveryCertificate(node, qc, recoveryQc)
    /\ node \in AsyncCurrentResponsiveVoters
    /\ qc.context = context
    /\ ~BodyHeldBy(durableBodies, node, qc.context,
                    qc.view, qc.subject)
    => \E source \in
         (recoveryQc.signers \cap AsyncCurrentResponsiveVoters) \ {node}:
         BodyHeldBy(durableBodies, source, qc.context,
                    qc.view, qc.subject)
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                NEW qc \in QcRecordSet,
                NEW recoveryQc \in QcRecordSet,
                StrongInductiveInvariant,
                DecisionRecoveryCertificate(node, qc, recoveryQc),
                node \in AsyncCurrentResponsiveVoters,
                qc.context = context,
                ~BodyHeldBy(durableBodies, node, qc.context,
                             qc.view, qc.subject)
         PROVE \E source \in
                   (recoveryQc.signers
                      \cap AsyncCurrentResponsiveVoters) \ {node}:
                 BodyHeldBy(durableBodies, source, qc.context,
                            qc.view, qc.subject)
    <2>1. recoveryQc \in commitQCs
      BY <1>1, SMT
         DEF StrongInductiveInvariant, Safety, DecisionAgreement,
             DecisionRecoveryCertificate
    <2>2. /\ ModelConfiguration
           /\ QuorumConfiguration
           /\ CertificatesBackedByIntents
           /\ HonestIntentSound(
                commitIntents, durableBodies, ValidSubjects)
      BY <1>1
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             HonestDurableIntentsSound, Safety, TypeInvariant,
             ModelConfiguration
    <2>3. /\ recoveryQc.context = context
           /\ recoveryQc.context.epoch = CurrentEpoch
           /\ CurrentEpoch \in Epochs
      BY <1>1, <2>2, TypeInvariantMakesCurrentEpochTyped
         DEF DecisionRecoveryCertificate, CurrentEpoch,
             StrongInductiveInvariant, Safety
    <2>4. CertificateBackedBy(
             CurrentEpoch, recoveryQc, commitIntents)
      BY <2>1, <2>2, <2>3
         DEF CertificatesBackedByIntents
    <2>5. /\ DualQuorum(CurrentEpoch, recoveryQc.signers)
           /\ recoveryQc.signers
                \in SUBSET VotingRoster(CurrentEpoch)
      BY <2>4 DEF CertificateBackedBy, DualQuorum, CountQuorum
    <2>6. /\ DualQuorum(
                CurrentEpoch, AsyncCurrentResponsiveVoters)
           /\ AsyncCurrentResponsiveVoters
                \in SUBSET VotingRoster(CurrentEpoch)
      BY <2>2, <2>3, Isa
         DEF ModelConfiguration, AsyncCurrentResponsiveVoters,
             CurrentVoters
    <2>7. DualQuorumIntersectionHasHonest
      BY <2>2, DualQuorumHonestIntersection
    <2>8. (recoveryQc.signers
             \cap AsyncCurrentResponsiveVoters \cap Honest) # {}
      BY <2>3, <2>5, <2>6, <2>7
         DEF DualQuorumIntersectionHasHonest
    <2>9. PICK source \in
                recoveryQc.signers
                  \cap AsyncCurrentResponsiveVoters \cap Honest:
             TRUE
      BY <2>8
    <2>10. PICK vote \in commitIntents:
              VoteBacksCertificate(vote, recoveryQc, source)
      BY <2>4, <2>9 DEF CertificateBackedBy
    <2>11. /\ vote.signer = source
            /\ vote.context = recoveryQc.context
            /\ vote.view = recoveryQc.view
            /\ vote.subject = recoveryQc.subject
      BY <2>10 DEF VoteBacksCertificate
    <2>12. BodyHeldBy(durableBodies, source,
                      recoveryQc.context, recoveryQc.view,
                      recoveryQc.subject)
      BY <2>2, <2>9, <2>10, <2>11
         DEF HonestIntentSound
    <2>13. BodyHeldBy(durableBodies, source,
                      qc.context, qc.view, qc.subject)
      BY <1>1, <2>12 DEF DecisionRecoveryCertificate
    <2>14. source # node
      BY <1>1, <2>13
    <2> QED BY <2>9, <2>13, <2>14
  <1> QED BY <1>1

THEOREM DecisionSourceRetentionProvidesDurableDecisionWitness ==
  DecisionSourceRetentionInvariant => DurableDecisionProgressWitness
PROOF
  <1>1. ASSUME DecisionSourceRetentionInvariant
         PROVE DurableDecisionProgressWitness
    <2>1. ASSUME NEW decision \in decisions,
                  /\ decision.node \in AsyncCurrentResponsiveVoters
                  /\ decision.qc.context = context
           PROVE DecisionCompletionWitness(decision.node, decision.qc)
      <3>1. DecisionRecoveryStage(decision.node, decision.qc)
        BY <1>1, <2>1 DEF DecisionSourceRetentionInvariant
      <3>2. NodeHasApplication(decision.node)
               => DecisionCompletionWitness(decision.node, decision.qc)
        BY DEF DecisionCompletionWitness
      <3>3. DecisionCertifiedRequestActive(
                 decision.node, decision.qc)
               => DecisionCompletionWitness(decision.node, decision.qc)
        BY Isa
           DEF DecisionCertifiedRequestActive,
               DecisionCompletionWitness
      <3>4. \A kind \in
                  {"RequestCertifiedBody", "StoreBody",
                   "ValidateBody", "Apply"}:
               DecisionCandidateOwned(decision.node, decision.qc, kind)
                 => DecisionCompletionWitness(
                      decision.node, decision.qc)
        <4>1. ASSUME NEW kind \in
                          {"RequestCertifiedBody", "StoreBody",
                           "ValidateBody", "Apply"},
                      DecisionCandidateOwned(
                        decision.node, decision.qc, kind)
               PROVE DecisionCompletionWitness(
                       decision.node, decision.qc)
          <5>1. PICK candidate \in AsyncCandidateSet:
                   /\ candidate.kind = kind
                   /\ DecisionPipelineCandidate(
                        decision.node, decision.qc, candidate)
            BY <4>1
               DEF DecisionCandidateOwned, DecisionPipelineKindOwned
          <5> QED BY <5>1 DEF DecisionCompletionWitness
        <4> QED BY <4>1
      <3>5. DecisionFetchBodyOwned(decision.node, decision.qc)
               => DecisionCompletionWitness(decision.node, decision.qc)
        BY Isa
           DEF DecisionFetchBodyOwned, DecisionCompletionWitness
      <3>6. DecisionCertifiedFetchOwned(decision.node, decision.qc)
               => DecisionCompletionWitness(decision.node, decision.qc)
        <4>1. ASSUME DecisionCertifiedFetchOwned(
                        decision.node, decision.qc)
               PROVE DecisionCompletionWitness(
                       decision.node, decision.qc)
          <5>1. PICK item \in AsyncNetworkItems,
                       recoveryQc \in QcRecordSet:
                   /\ item.envelope.recipient = decision.node
                   /\ item.envelope.height = decision.qc.context.height
                   /\ item.envelope.view = decision.qc.view
                   /\ item.envelope.subject = decision.qc.subject
                   /\ CertifiedResponseCandidate(item)
                        \in AsyncCandidateSet
                   /\ CandidateScheduled(
                        CertifiedResponseCandidate(item))
            BY <4>1 DEF DecisionCertifiedFetchOwned
          <5>2. DecisionPipelineCandidate(
                   decision.node, decision.qc,
                   CertifiedResponseCandidate(item))
            BY <5>1, Isa
               DEF DecisionPipelineCandidate,
                   CandidateConsumerCurrent,
                   CertifiedResponseCandidate, AsyncCandidate,
                   AsyncCandidateWithIdentity
          <5> QED BY <5>1, <5>2 DEF DecisionCompletionWitness
        <4> QED BY <4>1
      <3>7. DecisionRecoveryStage(decision.node, decision.qc)
               => DecisionCompletionWitness(decision.node, decision.qc)
        BY <3>2, <3>3, <3>4, <3>5, <3>6, SMT
           DEF DecisionRecoveryStage
      <3> QED BY <3>1, <3>7
    <2> QED BY <2>1 DEF DurableDecisionProgressWitness
  <1> QED BY <1>1

THEOREM DecisionRecoveryStageProvidesCompletionWitness ==
  \A node, qc:
    DecisionRecoveryStage(node, qc)
      => DecisionCompletionWitness(node, qc)
BY Isa
   DEF DecisionRecoveryStage, DecisionCompletionWitness,
       DecisionFetchBodyOwned, DecisionCertifiedRequestActive,
       DecisionCandidateOwned, DecisionCertifiedFetchOwned,
       DecisionPipelineCandidate, CandidateConsumerCurrent,
       DecisionPipelineKindOwned

THEOREM AsyncDecisionSourceRetentionProvidesRecoveryAwareWitness ==
  AsyncDecisionSourceRetentionInvariant
    => AsyncDurableDecisionProgressWitness
PROOF
  <1>1. ASSUME AsyncDecisionSourceRetentionInvariant
         PROVE AsyncDurableDecisionProgressWitness
    <2>1. ASSUME NEW decision \in decisions,
                  /\ decision.node \in AsyncCurrentResponsiveVoters
                  /\ decision.qc.context = context
           PROVE AsyncDecisionCompletionWitness(
                   decision.node, decision.qc)
      <3>1. AsyncDecisionRecoveryStage(
               decision.node, decision.qc)
        BY <1>1, <2>1 DEF AsyncDecisionSourceRetentionInvariant
      <3>2. DecisionRecoveryStage(decision.node, decision.qc)
               => DecisionCompletionWitness(
                    decision.node, decision.qc)
        BY DecisionRecoveryStageProvidesCompletionWitness
      <3> QED BY <3>1, <3>2
           DEF AsyncDecisionRecoveryStage,
               AsyncDecisionCompletionWitness
    <2> QED BY <2>1 DEF AsyncDurableDecisionProgressWitness
  <1> QED BY <1>1

THEOREM AsyncInitEstablishesDecisionSourceRetention ==
  \A initialContext:
    AsyncInitAt(initialContext) => DecisionSourceRetentionInvariant
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncInitAt(initialContext)
         PROVE DecisionSourceRetentionInvariant
    <2>1. /\ ModelConfiguration
           /\ FrozenContextAdmissible(initialContext)
           /\ initialContext.height \in Nat
           /\ context = initialContext
           /\ (initialContext.height = 0 => decisions = {})
           /\ (initialContext.height > 0
                 => /\ decisions =
                          {BootstrapParentDecision(initialContext)}
                    /\ BootstrapParentContext(initialContext)
                         # initialContext)
      <3>1. /\ ModelConfiguration
             /\ FrozenContextAdmissible(initialContext)
             /\ context = initialContext
             /\ (initialContext.height = 0 => decisions = {})
             /\ (initialContext.height > 0
                   => decisions =
                        {BootstrapParentDecision(initialContext)})
        BY <1>1, Isa DEF AsyncInitAt, AsyncBaseInitAt, InitAt
      <3>2. initialContext.height \in Nat
        BY <3>1, FrozenContextFieldsTyped DEF Heights
      <3>3. initialContext.height > 0
               => BootstrapParentContext(initialContext) # initialContext
        BY <3>1, BootstrapParentContextPrecedes
      <3> QED BY <3>1, <3>2, <3>3
    <2>2. ASSUME NEW decision \in decisions,
                  /\ decision.node \in AsyncCurrentResponsiveVoters
                  /\ decision.qc.context = context
           PROVE DecisionRecoveryStage(decision.node, decision.qc)
      <3>1. CASE initialContext.height = 0
        BY <2>1, <2>2, <3>1
      <3>2. CASE initialContext.height > 0
        <4>1. decision = BootstrapParentDecision(initialContext)
          BY <2>1, <2>2, <3>2
        <4>2. decision.qc.context =
                 BootstrapParentContext(initialContext)
          BY <4>1
             DEF BootstrapParentDecision, BootstrapParentCommitQC, QC
        <4>3. decision.qc.context = initialContext
          BY <2>1, <2>2
        <4> QED BY <2>1, <3>2, <4>2, <4>3
      <3> QED BY <2>1, <3>1, <3>2, SMT
    <2> QED BY <2>2 DEF DecisionSourceRetentionInvariant
  <1> QED BY <1>1

(***************************************************************************
Actions outside the serialized recovery reducer preserve every exact
decision-stage witness.  The frame is intentionally narrower than a blanket
scheduler stutter: producer and causal admission may move an owner between
queues, while authenticated ingress may add unrelated work.  It also fixes the
consumer context, view, and generation; a transition that advances a consumer
epoch must reconstruct a current Decision owner and needs a separate
preservation proof.  Only certified body requests and already-scheduled
current-consumer recovery candidates must survive this frame.  The signed
response history is append-only rather than unchanged: preserving every old
authentication occurrence is sufficient while still allowing unrelated
service responses to be published.
***************************************************************************)

DecisionCertifiedRequestsRetained ==
  \A request \in asyncActiveRequests:
    request.kind = "CertifiedRequest" => request \in asyncActiveRequests'

DecisionRetentionFrame ==
  /\ UNCHANGED <<context, nodeView, generation, decisions, applied,
                 availableBodies, durableBodies, validatedBodies>>
  /\ AsyncCurrentResponsiveVoters'
       \subseteq AsyncCurrentResponsiveVoters
  /\ asyncSentItems \subseteq asyncSentItems'
  /\ DecisionCertifiedRequestsRetained
  /\ \A candidate \in AsyncCandidateSet:
       CandidateScheduled(candidate) => CandidateScheduled(candidate)'

THEOREM TailRetainsNonHeadValue ==
  \A sequence, value:
    /\ sequence \in Seq(Range(sequence))
    /\ Len(sequence) > 0
    /\ value \in SequenceSet(sequence)
    /\ value # Head(sequence)
    => value \in SequenceSet(Tail(sequence))
PROOF
  <1>1. ASSUME NEW sequence, NEW value,
                sequence \in Seq(Range(sequence)),
                Len(sequence) > 0,
                value \in SequenceSet(sequence),
                value # Head(sequence)
         PROVE value \in SequenceSet(Tail(sequence))
    <2>1. PICK original \in 1..Len(sequence):
             value = sequence[original]
      BY <1>1 DEF SequenceSet
    <2>2. /\ sequence # <<>>
           /\ Head(sequence) = sequence[1]
           /\ Tail(sequence) \in Seq(Range(sequence))
           /\ Len(Tail(sequence)) = Len(sequence) - 1
           /\ \A index \in 1..Len(Tail(sequence)):
                Tail(sequence)[index] = sequence[index + 1]
      BY <1>1, PositiveSequenceIsNonempty,
         NonemptySequenceHeadIsFirst, HeadTailProperties
    <2>3. original - 1 \in 1..Len(Tail(sequence))
      BY <1>1, <2>1, <2>2, SMT
    <2>4. Tail(sequence)[original - 1] = value
      BY <2>1, <2>2, <2>3, SMT
    <2> QED BY <2>3, <2>4 DEF SequenceSet
  <1> QED BY <1>1

THEOREM NaturalPredecessorAfter ==
  \A low, high \in Nat:
    low > high
      => /\ ~(low - 1 < high)
         /\ (low - 1) + 1 = low
BY SMT

THEOREM SequenceWithoutIndexRetainsOtherValue ==
  \A sequence, index, value:
    /\ sequence \in Seq(Range(sequence))
    /\ index \in 1..Len(sequence)
    /\ value \in SequenceSet(sequence)
    /\ value # sequence[index]
    => value \in SequenceSet(SequenceWithoutIndex(sequence, index))
PROOF
  <1>1. ASSUME NEW sequence, NEW index, NEW value,
                sequence \in Seq(Range(sequence)),
                index \in 1..Len(sequence),
                value \in SequenceSet(sequence),
                value # sequence[index]
         PROVE value \in
                 SequenceSet(SequenceWithoutIndex(sequence, index))
    <2> DEFINE Result == SequenceWithoutIndex(sequence, index)
    <2>1. PICK original \in 1..Len(sequence):
             value = sequence[original]
      BY <1>1 DEF SequenceSet
    <2>2. /\ Len(Result) = Len(sequence) - 1
           /\ \A resultIndex \in 1..Len(Result):
                Result[resultIndex] =
                  IF resultIndex < index
                  THEN sequence[resultIndex]
                  ELSE sequence[resultIndex + 1]
      BY <1>1, SequenceWithoutIndexFacts DEF Result
    <2>3. CASE original < index
      <3>1. original \in 1..Len(Result)
        BY <1>1, <2>1, <2>2, <2>3, SMT
      <3>2. Result[original] = value
        BY <2>1, <2>2, <2>3, <3>1
      <3> QED BY <3>1, <3>2 DEF SequenceSet
    <2>4. CASE original > index
      <3>1. original - 1 \in 1..Len(Result)
        BY <1>1, <2>1, <2>2, <2>4, SMT
      <3>2. /\ original \in Nat
             /\ index \in Nat
        BY <1>1, <2>1, Isa
      <3>3. /\ ~(original - 1 < index)
             /\ (original - 1) + 1 = original
        BY <2>4, <3>2, NaturalPredecessorAfter
      <3>4. Result[original - 1] =
               IF original - 1 < index
               THEN sequence[original - 1]
               ELSE sequence[(original - 1) + 1]
        BY <2>2, <3>1
      <3>5. Result[original - 1] =
               sequence[(original - 1) + 1]
        BY <3>3, <3>4
      <3>6. Result[original - 1] = value
        BY <2>1, <3>3, <3>5
      <3> QED BY <3>1, <3>6 DEF SequenceSet
    <2> QED BY <1>1, <2>1, <2>3, <2>4, SMT
  <1> QED BY <1>1

THEOREM DecisionRetentionFramePreservesDecisionSourceRetention ==
  /\ DecisionSourceRetentionInvariant
  /\ DecisionRetentionFrame
  => DecisionSourceRetentionInvariant'
PROOF
  <1>1. ASSUME DecisionSourceRetentionInvariant,
                DecisionRetentionFrame
         PROVE DecisionSourceRetentionInvariant'
    <2>1. \A node, qc:
             NodeHasApplication(node) => NodeHasApplication(node)'
      BY <1>1, Isa
         DEF DecisionRetentionFrame, NodeHasApplication
    <2>2. \A node, qc:
             DecisionCertifiedRequestActive(node, qc)
               => DecisionCertifiedRequestActive(node, qc)'
      BY <1>1, Isa
         DEF DecisionRetentionFrame,
             DecisionCertifiedRequestsRetained,
             DecisionCertifiedRequestActive,
             DecisionRecoveryCertificate
    <2>3. \A node, qc, kind:
             DecisionCandidateOwned(node, qc, kind)
               => DecisionCandidateOwned(node, qc, kind)'
      BY <1>1, Isa
         DEF DecisionRetentionFrame, DecisionCandidateOwned
    <2>4. \A node, qc:
             DecisionFetchBodyOwned(node, qc)
               => DecisionFetchBodyOwned(node, qc)'
      BY <1>1, Isa
         DEF DecisionRetentionFrame, DecisionFetchBodyOwned,
             DecisionPipelineCandidate, CandidateConsumerCurrent
    <2>5. \A node, qc:
             DecisionCertifiedFetchOwned(node, qc)
               => DecisionCertifiedFetchOwned(node, qc)'
      <3>1. \A item:
               CertifiedResponseAuthenticatedOccurrence(item)
                 => CertifiedResponseAuthenticatedOccurrence(item)'
        BY <1>1, Isa
           DEF DecisionRetentionFrame,
               CertifiedResponseAuthenticatedOccurrence,
               AsyncCertifiedResponseAuthProjection
      <3> QED BY <1>1, <3>1, Isa
           DEF DecisionRetentionFrame, DecisionCertifiedFetchOwned,
               DecisionRecoveryCertificate
    <2>6. ASSUME NEW decision \in decisions',
                  /\ decision.node \in AsyncCurrentResponsiveVoters'
                     /\ decision.qc.context = context'
           PROVE DecisionRecoveryStage(decision.node, decision.qc)'
      <3>1. /\ decision \in decisions
             /\ decision.node \in AsyncCurrentResponsiveVoters
             /\ decision.qc.context = context
        BY <1>1, <2>6, Isa DEF DecisionRetentionFrame
      <3>2. DecisionRecoveryStage(decision.node, decision.qc)
        BY <1>1, <3>1 DEF DecisionSourceRetentionInvariant
      <3> QED BY <1>1, <2>1, <2>2, <2>3, <2>4, <2>5, <3>2, Isa
           DEF DecisionRetentionFrame, DecisionRecoveryStage,
               DecisionFetchBodyOwned, DecisionBody,
               DecisionValidationHeld
    <2> QED BY <2>6 DEF DecisionSourceRetentionInvariant
  <1> QED BY <1>1

THEOREM AsyncAllVarsStutterPreservesDurableCommitProgressWitness ==
  /\ DurableCommitProgressWitness
  /\ UNCHANGED AsyncAllVars
  => DurableCommitProgressWitness'
BY Isa
   DEF DurableCommitProgressWitness, CommitIntentProgressWitness,
       ActiveLockedCommitIntent, RetainedCommitIntent,
       NodeHasDecision, AsyncCurrentResponsiveVoters,
       CurrentVoters, CurrentEpoch, AsyncAllVars,
       AsyncSchedulerVars, vars

THEOREM AsyncAllVarsStutterPreservesDurableDecisionProgressWitness ==
  /\ DurableDecisionProgressWitness
  /\ UNCHANGED AsyncAllVars
  => DurableDecisionProgressWitness'
BY Isa
   DEF DurableDecisionProgressWitness, DecisionCompletionWitness,
       DecisionPipelineCandidate, CandidateConsumerCurrent,
       CandidateScheduled,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, SequenceSet, NodeHasApplication,
       AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch,
       AsyncAllVars, AsyncSchedulerVars, vars

THEOREM AsyncAllVarsStutterPreservesProtectedDeferredProgressInvariant ==
  /\ ProtectedDeferredProgressInvariant
  /\ UNCHANGED AsyncAllVars
  => ProtectedDeferredProgressInvariant'
PROOF
  <1>1. ASSUME ProtectedDeferredProgressInvariant,
                UNCHANGED AsyncAllVars
         PROVE ProtectedDeferredProgressInvariant'
    <2>1. /\ context' = context
           /\ nodeView' = nodeView
           /\ prepareQCs' = prepareQCs
           /\ lockRank' = lockRank
           /\ lockSubject' = lockSubject
           /\ asyncDeferredProgressQueues' =
                asyncDeferredProgressQueues
      BY <1>1, Isa
         DEF AsyncAllVars, AsyncSchedulerVars, vars
    <2>2. \A node \in ValidatorIds:
             ProtectedDeferredProgressIndices(node)' =
               ProtectedDeferredProgressIndices(node)
      BY <2>1
         DEF ProtectedDeferredProgressIndices,
             ProtectedProgressCommand, HistoricalLockedCommitItem,
             LockedPrepareRound
    <2>3. \A node \in ValidatorIds:
             ProtectedDeferredProgressCardinality(node)' =
               ProtectedDeferredProgressCardinality(node)
      BY <2>2 DEF ProtectedDeferredProgressCardinality
    <2>4. \A node, left, right:
             ProtectedDeferredProgressSlot(node, left, right)' =
               ProtectedDeferredProgressSlot(node, left, right)
      BY <2>1
         DEF ProtectedDeferredProgressSlot,
             SameProtectedProgressSlot, ProtectedProgressCommand,
             HistoricalLockedCommitItem, LockedPrepareRound
    <2>5. \A node \in ValidatorIds:
             ProtectedDeferredProgressUniqueness(node)' =
               ProtectedDeferredProgressUniqueness(node)
      BY <2>2, <2>4 DEF ProtectedDeferredProgressUniqueness
    <2>6. \A node \in ValidatorIds:
             ProtectedDeferredProgressNode(node)
      BY <1>1, Isa
         DEF ProtectedDeferredProgressInvariant,
             ProtectedDeferredProgressNode,
             ProtectedDeferredProgressCardinality,
             ProtectedDeferredProgressUniqueness,
             ProtectedDeferredProgressSlot
    <2>7. \A node \in ValidatorIds:
             ProtectedDeferredProgressNode(node)'
      BY <2>3, <2>5, <2>6
         DEF ProtectedDeferredProgressNode
    <2> QED BY <2>7,
         PrimedProtectedDeferredProgressNodesImplyInvariant
  <1> QED BY <1>1

THEOREM AsyncAllVarsStutterPreservesProgressWitness ==
  /\ ProgressWitnessInvariant
  /\ UNCHANGED AsyncAllVars
  => ProgressWitnessInvariant'
PROOF
  <1>1. ASSUME ProgressWitnessInvariant,
                UNCHANGED AsyncAllVars
         PROVE ProgressWitnessInvariant'
    <2>1. DurableCommitProgressWitness'
      BY <1>1,
         AsyncAllVarsStutterPreservesDurableCommitProgressWitness
         DEF ProgressWitnessInvariant
    <2>2. HistoricalLockedCommitRecoveryProgress'
      BY <1>1, Isa
         DEF ProgressWitnessInvariant,
             HistoricalLockedCommitRecoveryProgress,
             HistoricalLockedCommitRecoveryWitness,
             HistoricalBeginLockRecoveryCandidate,
             HistoricalBeginLockRecoveryEvidence,
             HistoricalCertifiedResponseRecoveryEvidence,
             CertifiedResponseAuthenticatedOccurrence,
             AsyncCertifiedResponseAuthProjection,
             CertifiedArchiveRoutes,
             AsyncCertifiedRequestHashOf,
             AsyncCertifiedSignedRequest,
             AsyncCertifiedRequestPreimage,
             AsyncCertifiedRequestSignature,
             CurrentVoters, CurrentEpoch,
             SamePrepareRecoveryRef, SameCertificateRef,
             CertificateRefOf,
             HistoricalLockedPrepareForCommit,
             InstalledTcSelectsPrepareFor,
             NoHigherPrepareOriginKnown, CandidateScheduled,
             AsyncAllVars
    <2>3. DurableDecisionProgressWitness'
      BY <1>1,
         AsyncAllVarsStutterPreservesDurableDecisionProgressWitness
         DEF ProgressWitnessInvariant
    <2>4. ProtectedDeferredProgressInvariant'
      BY <1>1,
         AsyncAllVarsStutterPreservesProtectedDeferredProgressInvariant
         DEF ProgressWitnessInvariant
    <2> QED BY <2>1, <2>2, <2>3, <2>4
         DEF ProgressWitnessInvariant
  <1> QED BY <1>1

(***************************************************************************
Recovery-aware progress-witness closure.

The two inductions below deliberately stay below every temporal fairness and
rank theorem.  The historical-lock leaf preserves the exact post-validation
BeginLockCommit/WAL frontier; the Decision leaf preserves the exact
current-consumer recovery owner and uses the durable crash authority only
across the responsive crash/restart/replay reset.  Their supporting
invariants are already independently inductive, so this cone cannot consume
starvation freedom, productive deadlock freedom, application liveness, or the
locked-body temporal reproposal obligation which later depend on it.
***************************************************************************)

THEOREM DurableDecisionProgressWitnessProjectsRecoveryAware ==
  DurableDecisionProgressWitness => AsyncDurableDecisionProgressWitness
BY Isa
   DEF DurableDecisionProgressWitness,
       AsyncDurableDecisionProgressWitness,
       AsyncDecisionCompletionWitness

THEOREM AsyncAllVarsStutterPreservesRecoveryAwareDecisionProgressWitness ==
  /\ AsyncDurableDecisionProgressWitness
  /\ UNCHANGED AsyncAllVars
  => AsyncDurableDecisionProgressWitness'
BY Isa
   DEF AsyncDurableDecisionProgressWitness,
       AsyncDecisionCompletionWitness,
       DecisionCompletionWitness, DecisionRecoveryAuthority,
       DurableDecisionRecoveryAuthority,
       DurableDecisionRecoveryExecutorCurrent,
       DecisionPipelineCandidate, CandidateConsumerCurrent,
       CandidateScheduled, QueuedCandidates, DeferredCandidates,
       CausalCandidates, TrackedWorkCandidates, SequenceSet,
       NodeHasApplication, AsyncCurrentResponsiveVoters,
       CurrentVoters, CurrentEpoch, RestartDecisions,
       AsyncAllVars, AsyncSchedulerVars, AsyncRecoveryVars, vars

THEOREM AsyncNextPreservesRecoveryAwareDecisionProgressWitness ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncProgressOwnershipInvariant
  /\ DecisionTimeoutFrontierInvariant
  /\ DecisionFrontierUniquenessInvariant
  /\ AsyncDurableDecisionProgressWitness
  /\ AsyncNext
  => AsyncDurableDecisionProgressWitness'
BY PersistDecisionRecoveryUsesBodyStateCompletion,
   CompletionDeferralRetainsCandidate,
   ExactDurableDecisionRecoveryLifecycleTransition,
   UniqueDecisionRestartDecisionReplayIsExactCurrentFetch,
   IsaT(600)
   DEF AsyncDurableDecisionProgressWitness,
       AsyncDecisionCompletionWitness, DecisionCompletionWitness,
       DecisionRecoveryAuthority, DecisionPipelineCandidate,
       CandidateConsumerCurrent, CandidateScheduled,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, DecisionTimeoutFrontierInvariant,
       DecisionFrontierUniquenessInvariant,
       DecisionsUniqueByNodeContext, AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncOutstandingCarrierInvariant,
       AsyncNext, AsyncNonCrashStep, AsyncRunnerStep,
       AsyncNonRunnerStep, RunNode, RunHistoricalRecoveryNode,
       AsyncEnterIndexedServiceActivation, AsyncActivateServiceNode,
       AsyncServiceActivationFrameVars,
       RunNodeWork, RunHistoricalServer, OpenHistoricalRecovery,
       ResolveRunNodeCandidateProducerContinuation,
       ReplayRunNodeCandidateProducerContinuation,
       AsyncCandidateProducerContinuationExactLocalReplayStep,
       AsyncCandidateProducerContinuationReplayTargetOnlyTurn,
       AsyncCandidateProducerContinuationExactRuntimeReplayStep,
       AsyncCandidateProducerContinuationExactReplayIdentity,
       AsyncCandidateProducerContinuationSelectedLocalCandidate,
       AsyncCandidateProducerContinuationSelectedRuntimeCandidate,
       AsyncCandidateProducerContinuationSelectedReplayRecord,
       AsyncCandidateProducerContinuationSelectedResolutionRecord,
       AsyncCandidateProducerContinuationResolutionRequired,
       AsyncCandidateProducerContinuationResolutionReady,
       AsyncCandidateProducerContinuationResolutionRecordsForNode,
       AsyncCandidateProducerContinuationConcreteSuccessorOwned,
       AsyncCandidateProducerContinuationHandoffOwned,
       AsyncCandidateProducerContinuationLocalReplayCarrier,
       AsyncSchedulerExceptCausalControlAndNodeService,
       AsyncSchedulerExceptCausalControlCommandRunnerAndNodeService,
       AsyncSchedulerExceptCausalControlRunnerAndNodeService,
       AsyncIoTimeoutLifecycleRetirementTransition,
       DirectCommitCertificateDiscoveryStep,
       DirectHistoricalCommitCertificateDiscoveryStep,
       ServiceIoWorker, ServiceHistoricalRecoveryIoWorker,
       EnqueueIoLocalControl, EnqueueHistoricalRecoveryIoLocalControl,
       AsyncNetworkStep, AdmitIngressPacket, AsyncFaultStep,
       PreGstCrash, PreGstResponsiveCrash, PreGstResponsiveRestart,
       PreGstResponsiveReplay, DriveResponsiveReplayHead,
       FinishResponsiveReplay, RearmResponsiveRecovery,
       LocalAdmissionStep, IngressDrainStep, SerializedRunnerRuntimeStep,
       SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       AsyncServeIngressTargetOnlyTurn,
       RuntimeStep, FifoRuntimeStep, DeferredDrainStep,
       ExecuteCommand, ExecuteRegularCommand, ExecuteDecisionFetch,
       ExecutePersistDecision, ExecuteRequestCertifiedBody,
       ExecuteApply, ExecuteCoreDelivery, ExecuteChunkDelivery,
       AppendCausalSuccessors, FreshCommandSuccessors,
       FreshCandidateSequence, CommandSuccessors,
       EnqueueCandidate,
       PersistDecisionRecoverySuccessor, PersistDecisionRecoveryKind,
       PersistDecisionBody, PersistDecisionValidationHeld,
       PersistDecisionRequest, PersistDecisionRequests,
       DrainFairIngressSelected, CertifiedResponseCandidate,
       PublishCertifiedRequests, CertifiedRequestOutbox,
       ResetNodeSchedulerForRestart, RestartReplay,
       RestartDecisionReplay, RestartCandidate,
       AsyncAllVars

THEOREM AsyncBracketPreservesRecoveryAwareDecisionProgressWitness ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncProgressOwnershipInvariant
  /\ DecisionTimeoutFrontierInvariant
  /\ DecisionFrontierUniquenessInvariant
  /\ AsyncDurableDecisionProgressWitness
  /\ [AsyncNext]_AsyncAllVars
  => AsyncDurableDecisionProgressWitness'
PROOF
  <1>1. CASE AsyncNext
    BY <1>1, AsyncNextPreservesRecoveryAwareDecisionProgressWitness
  <1>2. CASE UNCHANGED AsyncAllVars
    BY <1>2,
       AsyncAllVarsStutterPreservesRecoveryAwareDecisionProgressWitness
  <1> QED BY <1>1, <1>2

THEOREM RecoveryAwareDecisionProgressWitnessObligation ==
  \A initialContext:
    AsyncSpecAt(initialContext)
      => []AsyncDurableDecisionProgressWitness
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE AsyncSpecAt(initialContext)
                 => []AsyncDurableDecisionProgressWitness
    <2> DEFINE Inductive ==
           /\ AsyncStrongTypeInvariant
           /\ AsyncProgressOwnershipInvariant
           /\ DecisionTimeoutFrontierInvariant
           /\ DecisionFrontierUniquenessInvariant
           /\ AsyncDurableDecisionProgressWitness
    <2>1. AsyncInitAt(initialContext) => Inductive
      <3>1. AsyncInitAt(initialContext) => AsyncStrongTypeInvariant
        BY AsyncInitEstablishesStrongTypeInvariant
      <3>2. AsyncInitAt(initialContext)
               => AsyncProgressOwnershipInvariant
        BY AsyncInitEstablishesProgressOwnership
      <3>3. AsyncInitAt(initialContext)
               => DecisionTimeoutFrontierInvariant
        BY AsyncInitEstablishesDecisionTimeoutFrontier
      <3>4. AsyncInitAt(initialContext)
               => DecisionFrontierUniquenessInvariant
        BY AsyncInitEstablishesDecisionFrontierUniqueness
      <3>5. AsyncInitAt(initialContext)
               => DurableDecisionProgressWitness
        BY AsyncInitEstablishesProgressWitness
           DEF ProgressWitnessInvariant
      <3>6. AsyncInitAt(initialContext)
               => AsyncDurableDecisionProgressWitness
        BY <3>5, DurableDecisionProgressWitnessProjectsRecoveryAware
      <3> QED BY <3>1, <3>2, <3>3, <3>4, <3>6 DEF Inductive
    <2>2. Inductive /\ [AsyncNext]_AsyncAllVars => Inductive'
      BY AsyncBracketNextPreservesStrongTypeInvariant,
         AsyncBracketNextPreservesProgressOwnership,
         AsyncBracketPreservesDecisionTimeoutFrontier,
         AsyncBracketPreservesStrongDecisionFrontier,
         AsyncBracketPreservesRecoveryAwareDecisionProgressWitness
         DEF Inductive
    <2>3. AsyncSpecAt(initialContext) => []Inductive
      BY <2>1, <2>2, PTL DEF AsyncSpecAt
    <2>4. Inductive => AsyncDurableDecisionProgressWitness
      BY DEF Inductive
    <2> QED BY <2>3, <2>4, PTL
  <1> QED BY <1>1

THEOREM AsyncAllVarsStutterPreservesHistoricalLockedCommitRecoveryProgress ==
  /\ HistoricalLockedCommitRecoveryProgress
  /\ UNCHANGED AsyncAllVars
  => HistoricalLockedCommitRecoveryProgress'
BY Isa
   DEF HistoricalLockedCommitRecoveryProgress,
       HistoricalLockedCommitRecoveryWitness,
       HistoricalLockedPrepareForCommit,
       HistoricalLockedPrepareSource,
       HistoricalLockedPrepareRecoveryProvenance,
       InstalledTcSelectsPrepareFor, ExactLockedCommitIntents,
       NoHigherConflictingPrepareKnown, CandidateScheduled,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, SequenceSet,
       AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch,
       AsyncAllVars, AsyncSchedulerVars, vars

THEOREM AsyncNextPreservesHistoricalLockedCommitRecoveryProgress ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncProgressOwnershipInvariant
  /\ HistoricalLockedBodyRecoveryStageInvariant
  /\ HistoricalLockedCommitRecoveryProgress
  /\ AsyncNext
  => HistoricalLockedCommitRecoveryProgress'
BY HistoricalLockedValidateExecutionHandsOff,
   HistoricalLockedPersistInstallEstablishesSemanticFetch,
   HistoricalLockedPersistCommitEstablishesTerminalWitness,
   HistoricalLockedBodyExistingSourceStepPreservation,
   HistoricalLockedBodyNewSourceStepEstablishment,
   AsyncBracketNextPreservesProgressOwnership,
   IsaT(600)
   DEF HistoricalLockedCommitRecoveryProgress,
       HistoricalLockedCommitRecoveryWitness,
       HistoricalLockedPrepareForCommit,
       HistoricalLockedPrepareSource,
       HistoricalLockedPrepareRecoveryProvenance,
       HistoricalLockedBodyRecoveryStageInvariant,
       HistoricalLockedBodyRecoveryStage,
       HistoricalLockedBodyRecoveryTerminal,
       HistoricalLockedBodyValidated,
       InstalledTcSelectsPrepareFor, ExactLockedCommitIntents,
       NoHigherConflictingPrepareKnown, CandidateScheduled,
       AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncOutstandingCarrierInvariant,
       AsyncNext, AsyncNonCrashStep, AsyncRunnerStep,
       AsyncNonRunnerStep, RunNode, RunHistoricalRecoveryNode,
       AsyncEnterIndexedServiceActivation, AsyncActivateServiceNode,
       AsyncServiceActivationFrameVars,
       RunNodeWork, RunHistoricalServer, OpenHistoricalRecovery,
       ResolveRunNodeCandidateProducerContinuation,
       ReplayRunNodeCandidateProducerContinuation,
       AsyncCandidateProducerContinuationExactLocalReplayStep,
       AsyncCandidateProducerContinuationReplayTargetOnlyTurn,
       AsyncCandidateProducerContinuationExactRuntimeReplayStep,
       AsyncCandidateProducerContinuationExactReplayIdentity,
       AsyncCandidateProducerContinuationSelectedLocalCandidate,
       AsyncCandidateProducerContinuationSelectedRuntimeCandidate,
       AsyncCandidateProducerContinuationSelectedReplayRecord,
       AsyncCandidateProducerContinuationSelectedResolutionRecord,
       AsyncCandidateProducerContinuationResolutionRequired,
       AsyncCandidateProducerContinuationResolutionReady,
       AsyncCandidateProducerContinuationResolutionRecordsForNode,
       AsyncCandidateProducerContinuationConcreteSuccessorOwned,
       AsyncCandidateProducerContinuationHandoffOwned,
       AsyncCandidateProducerContinuationLocalReplayCarrier,
       AsyncSchedulerExceptCausalControlAndNodeService,
       AsyncSchedulerExceptCausalControlCommandRunnerAndNodeService,
       AsyncSchedulerExceptCausalControlRunnerAndNodeService,
       AsyncIoTimeoutLifecycleRetirementTransition,
       DirectCommitCertificateDiscoveryStep,
       DirectHistoricalCommitCertificateDiscoveryStep,
       ServiceIoWorker, ServiceHistoricalRecoveryIoWorker,
       EnqueueIoLocalControl, EnqueueHistoricalRecoveryIoLocalControl,
       AsyncNetworkStep, AdmitIngressPacket, AsyncFaultStep,
       PreGstCrash, PreGstResponsiveCrash, PreGstResponsiveRestart,
       PreGstResponsiveReplay, DriveResponsiveReplayHead,
       FinishResponsiveReplay, RearmResponsiveRecovery,
       LocalAdmissionStep, IngressDrainStep, SerializedRunnerRuntimeStep,
       SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       AsyncServeIngressTargetOnlyTurn,
       RuntimeStep, FifoRuntimeStep, DeferredDrainStep,
       ExecuteCommand, ExecuteRegularCommand, ExecutePersistInstall,
       AppendCausalSuccessors, FreshCommandSuccessors,
       FreshCandidateSequence, CommandSuccessors,
       EnqueueCandidate,
       DrainFairIngressSelected, ResetNodeSchedulerForRestart,
       AsyncAllVars

THEOREM AsyncBracketPreservesHistoricalLockedCommitRecoveryProgress ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncProgressOwnershipInvariant
  /\ HistoricalLockedBodyRecoveryStageInvariant
  /\ HistoricalLockedCommitRecoveryProgress
  /\ [AsyncNext]_AsyncAllVars
  => HistoricalLockedCommitRecoveryProgress'
PROOF
  <1>1. CASE AsyncNext
    BY <1>1, AsyncNextPreservesHistoricalLockedCommitRecoveryProgress
  <1>2. CASE UNCHANGED AsyncAllVars
    BY <1>2,
       AsyncAllVarsStutterPreservesHistoricalLockedCommitRecoveryProgress
  <1> QED BY <1>1, <1>2

THEOREM HistoricalLockedCommitRecoveryProgressObligation ==
  \A initialContext:
    AsyncSpecAt(initialContext)
      => []HistoricalLockedCommitRecoveryProgress
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE AsyncSpecAt(initialContext)
                 => []HistoricalLockedCommitRecoveryProgress
    <2> DEFINE Inductive ==
           /\ AsyncStrongTypeInvariant
           /\ AsyncProgressOwnershipInvariant
           /\ HistoricalLockedBodyRecoveryStageInvariant
           /\ HistoricalLockedCommitRecoveryProgress
    <2>1. AsyncInitAt(initialContext) => Inductive
      <3>1. AsyncInitAt(initialContext) => AsyncStrongTypeInvariant
        BY AsyncInitEstablishesStrongTypeInvariant
      <3>2. AsyncInitAt(initialContext)
               => AsyncProgressOwnershipInvariant
        BY AsyncInitEstablishesProgressOwnership
      <3>3. AsyncInitAt(initialContext)
               => HistoricalLockedBodyRecoveryStageInvariant
        BY AsyncInitEstablishesHistoricalLockedBodyRecoveryStage
      <3>4. AsyncInitAt(initialContext)
               => HistoricalLockedCommitRecoveryProgress
        BY AsyncInitEstablishesProgressWitness
           DEF ProgressWitnessInvariant
      <3> QED BY <3>1, <3>2, <3>3, <3>4 DEF Inductive
    <2>2. Inductive /\ [AsyncNext]_AsyncAllVars => Inductive'
      BY AsyncBracketNextPreservesStrongTypeInvariant,
         AsyncBracketNextPreservesProgressOwnership,
         AsyncBracketPreservesHistoricalLockedBodyRecoveryStage,
         AsyncBracketPreservesHistoricalLockedCommitRecoveryProgress
         DEF Inductive
    <2>3. AsyncSpecAt(initialContext) => []Inductive
      BY <2>1, <2>2, PTL DEF AsyncSpecAt
    <2>4. Inductive => HistoricalLockedCommitRecoveryProgress
      BY DEF Inductive
    <2> QED BY <2>3, <2>4, PTL
  <1> QED BY <1>1

THEOREM AsyncSpecKeepsGstOnceSet ==
  \A initialContext:
    AsyncSpecAt(initialContext) => [](gst => []gst)
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE AsyncSpecAt(initialContext) => [](gst => []gst)
    <2>1. gst /\ [AsyncNext]_AsyncAllVars => gst'
      BY GstAsyncStepIsMonotone
    <2> QED BY <2>1, PTL DEF AsyncSpecAt
  <1> QED BY <1>1

=============================================================================
