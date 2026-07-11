---- MODULE SumeragiV2InductiveProofs ----
EXTENDS SumeragiV2Inductive, SumeragiV2SafetyLemmas

(***************************************************************************
Action-by-action proof that the executable reducer establishes and preserves
its asynchronous provenance.  This module is intentionally separate from the
TLC-loadable invariant vocabulary.
***************************************************************************)

THEOREM InitEstablishesTypeInvariant == Init => TypeInvariant
PROOF
  <1>1. ASSUME Init
         PROVE TypeInvariant
    <2>1. /\ ModelConfiguration
          /\ MaxHeight \in Nat
          /\ MaxView \in Nat
          /\ MaxGeneration \in Nat
      BY <1>1 DEF Init, ModelConfiguration
    <2>2. /\ 0 \in Heights
          /\ 0 \in Views
          /\ 0 \in Generations
          /\ NoRank \in Ranks
          /\ NoSubject \in SubjectOrNone
      BY <2>1, SMT
         DEF Heights, Views, Generations, Ranks, NoRank,
             SubjectOrNone, Subjects, NoSubject
    <2>3. ContextRecord(0, NoSubject) \in ContextRecords
      BY <2>2 DEF ContextRecords
    <2>4. /\ [node \in ValidatorIds |-> 0]
                    \in [ValidatorIds -> Views]
          /\ [node \in ValidatorIds |-> 0]
                    \in [ValidatorIds -> Generations]
          /\ [node \in ValidatorIds |-> NoRank]
                    \in [ValidatorIds -> Ranks]
          /\ [node \in ValidatorIds |-> NoSubject]
                    \in [ValidatorIds -> SubjectOrNone]
      BY <2>2, Isa
    <2>5. /\ context = ContextRecord(0, NoSubject)
          /\ context.height = 0
          /\ contextHistory = {context}
          /\ context \in contextHistory
          /\ contextHistory \subseteq ContextRecords
      BY <1>1, <2>3, Isa DEF Init, ContextRecord
    <2>6. /\ proposalIntents \subseteq ProposalRecordSet
          /\ prepareIntents \subseteq VoteRecordSet
          /\ commitIntents \subseteq VoteRecordSet
          /\ timeoutIntents \subseteq TimeoutVoteRecordSet
          /\ prepareQCs \subseteq QcRecordSet
          /\ commitQCs \subseteq QcRecordSet
          /\ \A tc \in formedTCs: TcWellTyped(tc)
          /\ \A entry \in receivedTCs:
               /\ entry.node \in ValidatorIds
               /\ TcWellTyped(entry.tc)
          /\ \A entry \in installedTCs:
               /\ entry.node \in ValidatorIds
               /\ TcWellTyped(entry.tc)
      BY <1>1, Isa DEF Init
    <2>7. /\ pendingProposal \subseteq ProposalWalSet
          /\ pendingPrepare \subseteq PrepareWalSet
          /\ pendingObservePrepare \subseteq ObservePrepareWalSet
          /\ pendingLockCommit \subseteq LockCommitWalSet
          /\ pendingTimeout \subseteq TimeoutWalSet
          /\ \A request \in pendingInstallTC:
               /\ request.node \in ValidatorIds
               /\ request.kind = "InstallTC"
               /\ TcWellTyped(request.tc)
               /\ request.rebroadcast \in BOOLEAN
          /\ pendingDecision \subseteq DecisionWalSet
          /\ signProposals \subseteq ProposalSignSet
          /\ signVotes \subseteq VoteSignSet
          /\ signTimeouts \subseteq TimeoutSignSet
      BY <1>1, Isa DEF Init
    <2>8. /\ height \in Heights
          /\ nodeView \in [ValidatorIds -> Views]
          /\ generation \in [ValidatorIds -> Generations]
          /\ up \subseteq ValidatorIds
          /\ gst \in BOOLEAN
          /\ lockRank \in [ValidatorIds -> Ranks]
          /\ lockSubject \in [ValidatorIds -> SubjectOrNone]
          /\ highestRank \in [ValidatorIds -> Ranks]
          /\ highestSubject \in [ValidatorIds -> SubjectOrNone]
      BY <1>1, <2>2, <2>4, Isa DEF Init
    <2> QED BY <1>1, <2>1, <2>5, <2>6, <2>7, <2>8
       DEF TypeInvariant, Init
  <1> QED BY <1>1

THEOREM InitEstablishesReleaseSafety == Init => Safety
PROOF
  <1>1. ASSUME Init
         PROVE Safety
    <2>1. TypeInvariant BY <1>1, InitEstablishesTypeInvariant
    <2>2. OnePendingPersistencePerNode
      BY <1>1, Isa
         DEF Init, OnePendingPersistencePerNode,
             RequestsUniqueByNode, AllPendingRequests
    <2>3. /\ ProposalSigningRequiresIntent
          /\ PrepareSigningRequiresIntent
          /\ CommitSigningRequiresIntent
          /\ TimeoutSigningRequiresIntent
          /\ HonestPrepareUniqueness
          /\ HonestCommitUniqueness
          /\ HonestTimeoutUniqueness
          /\ DecisionAgreement
          /\ AppliedRequiresDecision
      BY <1>1, Isa
         DEF Init, OnePendingPersistencePerNode,
             ProposalSigningRequiresIntent, PrepareSigningRequiresIntent,
             CommitSigningRequiresIntent, TimeoutSigningRequiresIntent,
             HonestPrepareUniqueness, HonestCommitUniqueness,
             HonestTimeoutUniqueness, DecisionAgreement,
             AppliedRequiresDecision
    <2>4. LockBelowHighest
      BY <1>1 DEF Init, LockBelowHighest, NoRank
    <2> QED BY <2>1, <2>2, <2>3, <2>4 DEF Safety
  <1> QED BY <1>1

THEOREM InitEstablishesReducerProvenance ==
  Init => ReducerProvenanceInvariant
PROOF
  <1>1. ASSUME Init
         PROVE ReducerProvenanceInvariant
    <2>1. /\ HonestVoteUnique(prepareIntents)
          /\ HonestVoteUnique(commitIntents)
          /\ HonestTimeoutUnique(timeoutIntents)
          /\ IntentPhasesCorrect
          /\ PendingVoteWritesAuthorized
          /\ PendingCertificateWritesAuthorized
          /\ HonestVoteTransportBacked
          /\ QcTransportBacked
          /\ HonestTimeoutTransportBacked
          /\ TcTransportBacked
          /\ CertificatesBackedByIntents
          /\ HonestDurableIntentsSound
          /\ FormedTimeoutCertificatesSound
          /\ DurableTimeoutsProtectCommits
      BY <1>1, Isa
         DEF Init, HonestVoteUnique, HonestTimeoutUnique,
             PendingVoteWritesAuthorized,
             PendingCertificateWritesAuthorized,
             HonestVoteTransportBacked, QcTransportBacked,
             HonestTimeoutTransportBacked, TcTransportBacked,
             CertificatesBackedByIntents, HonestDurableIntentsSound,
             HonestIntentSound, FormedTimeoutCertificatesSound,
             DurableTimeoutsProtectCommits,
             IntentPhasesCorrect,
             TimeoutIntentProtectsCommits
    <2>2. HighestAndLockAreCertified
      BY <1>1, Isa
         DEF Init, HighestAndLockAreCertified, NoRank, NoSubject
    <2> QED BY <2>1, <2>2 DEF ReducerProvenanceInvariant
  <1> QED BY <1>1

THEOREM InitEstablishesContextSafety ==
  Init
    => /\ ContextIdentityBindsFrozenEpoch
       /\ OldContextCertificateRejected
       /\ ContextParentWasApplied
PROOF
  <1>1. ASSUME Init
         PROVE /\ ContextIdentityBindsFrozenEpoch
               /\ OldContextCertificateRejected
               /\ ContextParentWasApplied
    <2>1. ContextIdentityBindsFrozenEpoch
      BY DEF ContextIdentityBindsFrozenEpoch, ContextRecords,
             ContextRecord, ExpectedEpoch
    <2>2. OldContextCertificateRejected
      BY <1>1, Isa DEF Init, OldContextCertificateRejected
    <2>3. ContextParentWasApplied
      <3>1. ASSUME NEW contextValue \in contextHistory,
                    contextValue.height > 0
             PROVE \E decision \in decisions:
                     /\ decision.qc.context.height + 1
                          = contextValue.height
                     /\ decision.qc.subject = contextValue.parent
                     /\ [node |-> decision.node, qc |-> decision.qc]
                          \in applied
        <4>1. contextValue = context
          BY <1>1, <3>1 DEF Init
        <4>2. contextValue.height = 0
          BY <1>1, <4>1 DEF Init, ContextRecord
        <4> QED BY <3>1, <4>2, SMT
      <3> QED BY <3>1 DEF ContextParentWasApplied
    <2> QED BY <2>1, <2>2, <2>3
  <1> QED BY <1>1

THEOREM InitEstablishesStrongInductiveInvariant ==
  Init => StrongInductiveInvariant
BY InitEstablishesReleaseSafety,
   InitEstablishesReducerProvenance,
   InitEstablishesContextSafety
   DEF StrongInductiveInvariant

THEOREM SetGSTPreservesStrongInductiveInvariant ==
  StrongInductiveInvariant /\ SetGST
    => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME StrongInductiveInvariant,
              SetGST
         PROVE StrongInductiveInvariant'
    <2>1. TypeInvariant'
      BY <1>1, SMT
         DEF StrongInductiveInvariant, Safety, TypeInvariant, SetGST
    <2>2. /\ OnePendingPersistencePerNode'
          /\ ProposalSigningRequiresIntent'
          /\ PrepareSigningRequiresIntent'
          /\ CommitSigningRequiresIntent'
          /\ TimeoutSigningRequiresIntent'
          /\ HonestPrepareUniqueness'
          /\ HonestCommitUniqueness'
          /\ HonestTimeoutUniqueness'
          /\ LockBelowHighest'
          /\ DecisionAgreement'
          /\ AppliedRequiresDecision'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, Safety, SetGST,
             OnePendingPersistencePerNode,
             RequestsUniqueByNode, AllPendingRequests,
             ProposalSigningRequiresIntent, PrepareSigningRequiresIntent,
             CommitSigningRequiresIntent, TimeoutSigningRequiresIntent,
             HonestPrepareUniqueness, HonestCommitUniqueness,
             HonestTimeoutUniqueness, LockBelowHighest, DecisionAgreement,
             AppliedRequiresDecision
    <2>3. Safety'
      BY <2>1, <2>2 DEF Safety
    <2>4. ContextIdentityBindsFrozenEpoch'
      BY <1>1
         DEF StrongInductiveInvariant, ContextIdentityBindsFrozenEpoch
    <2>5. OldContextCertificateRejected'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, SetGST,
             OldContextCertificateRejected, QcValid, CurrentEpoch
    <2>6. ContextParentWasApplied'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, SetGST, ContextParentWasApplied
    <2>7. ReducerProvenanceInvariant'
      <3>1. /\ HonestVoteUnique(prepareIntents)'
            /\ HonestVoteUnique(commitIntents)'
            /\ HonestTimeoutUnique(timeoutIntents)'
            /\ IntentPhasesCorrect'
        BY <1>1, Isa
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               SetGST, HonestVoteUnique, HonestTimeoutUnique,
               IntentPhasesCorrect
      <3>2. /\ PendingVoteWritesAuthorized'
            /\ PendingCertificateWritesAuthorized'
        BY <1>1, Isa
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               SetGST, PendingVoteWritesAuthorized,
               PendingCertificateWritesAuthorized
      <3>3. /\ HonestVoteTransportBacked'
            /\ QcTransportBacked'
            /\ HonestTimeoutTransportBacked'
            /\ TcTransportBacked'
        BY <1>1, Isa
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               SetGST, HonestVoteTransportBacked, QcTransportBacked,
               HonestTimeoutTransportBacked, TcTransportBacked,
               VoteIntentFor
      <3>4. /\ CertificatesBackedByIntents'
            /\ HonestDurableIntentsSound'
        BY <1>1, Isa
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               SetGST, CertificatesBackedByIntents,
               HonestDurableIntentsSound
      <3>5. /\ FormedTimeoutCertificatesSound'
            /\ DurableTimeoutsProtectCommits'
            /\ HighestAndLockAreCertified'
        BY <1>1, Isa
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               SetGST, FormedTimeoutCertificatesSound,
               DurableTimeoutsProtectCommits, HighestAndLockAreCertified
      <3> QED BY <3>1, <3>2, <3>3, <3>4, <3>5
         DEF ReducerProvenanceInvariant
    <2> QED BY <2>3, <2>4, <2>5, <2>6, <2>7
       DEF StrongInductiveInvariant
  <1> QED BY <1>1

THEOREM ProofRelevantStutterPreservesStrongInvariant ==
  StrongInductiveInvariant /\ UNCHANGED ProofRelevantVars
    => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME StrongInductiveInvariant,
              UNCHANGED ProofRelevantVars
         PROVE StrongInductiveInvariant'
    <2>1. Safety'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, ProofRelevantVars, Safety,
             TypeInvariant, OnePendingPersistencePerNode,
             RequestsUniqueByNode, AllPendingRequests,
             ProposalSigningRequiresIntent, PrepareSigningRequiresIntent,
             CommitSigningRequiresIntent, TimeoutSigningRequiresIntent,
             HonestPrepareUniqueness, HonestCommitUniqueness,
             HonestTimeoutUniqueness, LockBelowHighest, DecisionAgreement,
             AppliedRequiresDecision
    <2>2. /\ ContextIdentityBindsFrozenEpoch'
          /\ OldContextCertificateRejected'
          /\ ContextParentWasApplied'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, ProofRelevantVars,
             ContextIdentityBindsFrozenEpoch,
             OldContextCertificateRejected, ContextParentWasApplied,
             QcValid, CurrentEpoch
    <2>3. ReducerProvenanceInvariant'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, ProofRelevantVars,
             ReducerProvenanceInvariant, HonestVoteUnique,
             HonestTimeoutUnique, IntentPhasesCorrect,
             PendingVoteWritesAuthorized,
             PendingCertificateWritesAuthorized,
             HonestVoteTransportBacked, QcTransportBacked,
             HonestTimeoutTransportBacked, TcTransportBacked,
             CertificatesBackedByIntents, HonestDurableIntentsSound,
             FormedTimeoutCertificatesSound,
             DurableTimeoutsProtectCommits, HighestAndLockAreCertified,
             VoteIntentFor
    <2> QED BY <2>1, <2>2, <2>3 DEF StrongInductiveInvariant
  <1> QED BY <1>1

THEOREM DeliverProposalPreservesStrongInvariant ==
  \A envelope:
    StrongInductiveInvariant /\ DeliverProposal(envelope)
      => StrongInductiveInvariant'
BY ProofRelevantStutterPreservesStrongInvariant
   DEF DeliverProposal, ProofRelevantVars

THEOREM FetchBodyPreservesStrongInvariant ==
  \A node, proposal:
    StrongInductiveInvariant /\ FetchBody(node, proposal)
      => StrongInductiveInvariant'
BY ProofRelevantStutterPreservesStrongInvariant
   DEF FetchBody, ProofRelevantVars

THEOREM ValidateBodyPreservesStrongInvariant ==
  \A node, proposal:
    StrongInductiveInvariant /\ ValidateBody(node, proposal)
      => StrongInductiveInvariant'
BY ProofRelevantStutterPreservesStrongInvariant
   DEF ValidateBody, ProofRelevantVars

THEOREM RejectBodyPreservesStrongInvariant ==
  \A node, proposal:
    StrongInductiveInvariant /\ RejectBody(node, proposal)
      => StrongInductiveInvariant'
BY ProofRelevantStutterPreservesStrongInvariant
   DEF RejectBody, ProofRelevantVars

THEOREM FetchCertifiedBodyPreservesStrongInvariant ==
  \A node, qc:
    StrongInductiveInvariant /\ FetchCertifiedBody(node, qc)
      => StrongInductiveInvariant'
BY ProofRelevantStutterPreservesStrongInvariant
   DEF FetchCertifiedBody, ProofRelevantVars

THEOREM DropProposalPreservesStrongInvariant ==
  \A envelope:
    StrongInductiveInvariant /\ DropProposal(envelope)
      => StrongInductiveInvariant'
BY ProofRelevantStutterPreservesStrongInvariant
   DEF DropProposal, ProofRelevantVars

THEOREM HonestIntentSoundIsMonotoneInDurableBodies ==
  \A intents, before, after, validSubjects:
    HonestIntentSound(intents, before, validSubjects)
      /\ before \subseteq after
      => HonestIntentSound(intents, after, validSubjects)
BY DEF HonestIntentSound, BodyHeldBy

THEOREM DurableGrowthPreservesStrongInvariant ==
  StrongInductiveInvariant
    /\ durableBodies \subseteq durableBodies'
    /\ UNCHANGED ProofRelevantWithoutDurableVars
    => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME StrongInductiveInvariant,
              durableBodies \subseteq durableBodies',
              UNCHANGED ProofRelevantWithoutDurableVars
         PROVE StrongInductiveInvariant'
    <2>1. Safety'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, ProofRelevantWithoutDurableVars,
             Safety, TypeInvariant, OnePendingPersistencePerNode,
             RequestsUniqueByNode, AllPendingRequests,
             ProposalSigningRequiresIntent, PrepareSigningRequiresIntent,
             CommitSigningRequiresIntent, TimeoutSigningRequiresIntent,
             HonestPrepareUniqueness, HonestCommitUniqueness,
             HonestTimeoutUniqueness, LockBelowHighest, DecisionAgreement,
             AppliedRequiresDecision
    <2>2. /\ ContextIdentityBindsFrozenEpoch'
          /\ OldContextCertificateRejected'
          /\ ContextParentWasApplied'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, ProofRelevantWithoutDurableVars,
             ContextIdentityBindsFrozenEpoch,
             OldContextCertificateRejected, ContextParentWasApplied,
             QcValid, CurrentEpoch
    <2>3. HonestDurableIntentsSound'
      BY <1>1, HonestIntentSoundIsMonotoneInDurableBodies
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             HonestDurableIntentsSound,
             ProofRelevantWithoutDurableVars
    <2>4. /\ HonestVoteUnique(prepareIntents)'
          /\ HonestVoteUnique(commitIntents)'
          /\ HonestTimeoutUnique(timeoutIntents)'
          /\ IntentPhasesCorrect'
          /\ PendingVoteWritesAuthorized'
          /\ PendingCertificateWritesAuthorized'
          /\ HonestVoteTransportBacked'
          /\ QcTransportBacked'
          /\ HonestTimeoutTransportBacked'
          /\ TcTransportBacked'
          /\ CertificatesBackedByIntents'
          /\ FormedTimeoutCertificatesSound'
          /\ DurableTimeoutsProtectCommits'
          /\ HighestAndLockAreCertified'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             ProofRelevantWithoutDurableVars, HonestVoteUnique,
             HonestTimeoutUnique, IntentPhasesCorrect,
             PendingVoteWritesAuthorized,
             PendingCertificateWritesAuthorized,
             HonestVoteTransportBacked, QcTransportBacked,
             HonestTimeoutTransportBacked, TcTransportBacked,
             CertificatesBackedByIntents,
             FormedTimeoutCertificatesSound,
             DurableTimeoutsProtectCommits, HighestAndLockAreCertified,
             VoteIntentFor
    <2>5. ReducerProvenanceInvariant'
      BY <2>3, <2>4 DEF ReducerProvenanceInvariant
    <2> QED BY <2>1, <2>2, <2>5 DEF StrongInductiveInvariant
  <1> QED BY <1>1

THEOREM AssembleLocalBodyPreservesStrongInvariant ==
  \A node, subject:
    StrongInductiveInvariant /\ AssembleLocalBody(node, subject)
      => StrongInductiveInvariant'
BY DurableGrowthPreservesStrongInvariant
   DEF AssembleLocalBody, ProofRelevantWithoutDurableVars

THEOREM StoreBodyPreservesStrongInvariant ==
  \A node, subject:
    StrongInductiveInvariant /\ StoreBody(node, subject)
      => StrongInductiveInvariant'
BY DurableGrowthPreservesStrongInvariant
   DEF StoreBody, ProofRelevantWithoutDurableVars

THEOREM PendingNodesAreAllRequestNodes ==
  PendingNodes = RequestNodeSet(AllPendingRequests)
BY Isa DEF PendingNodes, RequestNodeSet, AllPendingRequests

THEOREM NewRequestPreservesNodeUniqueness ==
  \A requests, request:
    RequestsUniqueByNode(requests)
      /\ request.node \notin RequestNodeSet(requests)
      => RequestsUniqueByNode(requests \cup {request})
BY SMT DEF RequestsUniqueByNode, RequestNodeSet

THEOREM RemovingRequestsPreservesNodeUniqueness ==
  \A before, after:
    RequestsUniqueByNode(before) /\ after \subseteq before
      => RequestsUniqueByNode(after)
BY DEF RequestsUniqueByNode

THEOREM BeginLocalProposalPreservesStrongInvariant ==
  \A node, subject:
    StrongInductiveInvariant /\ BeginLocalProposal(node, subject)
      => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME NEW node,
              NEW subject,
              StrongInductiveInvariant,
              BeginLocalProposal(node, subject)
         PROVE StrongInductiveInvariant'
    <2>1. TypeInvariant'
      BY <1>1, SMT
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             BeginLocalProposal
    <2>2. OnePendingPersistencePerNode'
      <3>1. RequestsUniqueByNode(AllPendingRequests)
        BY <1>1
           DEF StrongInductiveInvariant, Safety,
               OnePendingPersistencePerNode
      <3>2. node \notin RequestNodeSet(AllPendingRequests)
        BY <1>1, PendingNodesAreAllRequestNodes
           DEF BeginLocalProposal, NodeIdle
      <3>3. AllPendingRequests'
               = AllPendingRequests
                   \cup {ProposalWal(node,
                                     LocalProposalFor(node, subject))}
        BY <1>1, Isa DEF BeginLocalProposal, AllPendingRequests
      <3>4. ProposalWal(node, LocalProposalFor(node, subject)).node = node
        BY DEF ProposalWal
      <3>5. RequestsUniqueByNode(
               AllPendingRequests
                 \cup {ProposalWal(node,
                                   LocalProposalFor(node, subject))})
        BY <3>1, <3>2, <3>4,
           NewRequestPreservesNodeUniqueness
      <3> QED BY <3>3, <3>5
         DEF OnePendingPersistencePerNode
    <2>3. /\ ProposalSigningRequiresIntent'
          /\ PrepareSigningRequiresIntent'
          /\ CommitSigningRequiresIntent'
          /\ TimeoutSigningRequiresIntent'
          /\ HonestPrepareUniqueness'
          /\ HonestCommitUniqueness'
          /\ HonestTimeoutUniqueness'
          /\ LockBelowHighest'
          /\ DecisionAgreement'
          /\ AppliedRequiresDecision'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, Safety, BeginLocalProposal,
             ProofRelevantWithoutPendingProposalVars,
             ProposalSigningRequiresIntent, PrepareSigningRequiresIntent,
             CommitSigningRequiresIntent, TimeoutSigningRequiresIntent,
             HonestPrepareUniqueness, HonestCommitUniqueness,
             HonestTimeoutUniqueness, LockBelowHighest, DecisionAgreement,
             AppliedRequiresDecision
    <2>4. Safety'
      BY <2>1, <2>2, <2>3 DEF Safety
    <2>5. /\ ContextIdentityBindsFrozenEpoch'
          /\ OldContextCertificateRejected'
          /\ ContextParentWasApplied'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, BeginLocalProposal,
             ProofRelevantWithoutPendingProposalVars,
             ContextIdentityBindsFrozenEpoch,
             OldContextCertificateRejected, ContextParentWasApplied,
             QcValid, CurrentEpoch
    <2>6. ReducerProvenanceInvariant'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, BeginLocalProposal,
             ProofRelevantWithoutPendingProposalVars,
             ReducerProvenanceInvariant, HonestVoteUnique,
             HonestTimeoutUnique, IntentPhasesCorrect,
             PendingVoteWritesAuthorized,
             PendingCertificateWritesAuthorized,
             HonestVoteTransportBacked, QcTransportBacked,
             HonestTimeoutTransportBacked, TcTransportBacked,
             CertificatesBackedByIntents, HonestDurableIntentsSound,
             FormedTimeoutCertificatesSound,
             DurableTimeoutsProtectCommits, HighestAndLockAreCertified,
             VoteIntentFor
    <2> QED BY <2>4, <2>5, <2>6 DEF StrongInductiveInvariant
  <1> QED BY <1>1

THEOREM PersistProposalPreservesStrongInvariant ==
  \A request:
    StrongInductiveInvariant /\ PersistProposal(request)
      => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME NEW request,
              StrongInductiveInvariant,
              PersistProposal(request)
         PROVE StrongInductiveInvariant'
    <2>1. TypeInvariant'
      <3>1. /\ request.node \in ValidatorIds
            /\ request.proposal \in ProposalRecordSet
        BY <1>1
           DEF StrongInductiveInvariant, Safety, TypeInvariant,
               PersistProposal, ProposalWalSet
      <3>2. proposalIntents \subseteq ProposalRecordSet
        BY <1>1 DEF StrongInductiveInvariant, Safety, TypeInvariant
      <3>3. proposalIntents' \subseteq ProposalRecordSet
        BY <1>1, <3>1, <3>2, Isa DEF PersistProposal
      <3>4. pendingProposal' \subseteq ProposalWalSet
        BY <1>1, Isa
           DEF StrongInductiveInvariant, Safety, TypeInvariant,
               PersistProposal
      <3>5. ProposalSign(request.node, request.proposal)
                 \in ProposalSignSet
        BY <3>1 DEF ProposalSign, ProposalSignSet
      <3>6. signProposals' \subseteq ProposalSignSet
        BY <1>1, <3>5, Isa
           DEF StrongInductiveInvariant, Safety, TypeInvariant,
               PersistProposal
      <3> QED BY <1>1, <3>3, <3>4, <3>6, Isa
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             PersistProposal
    <2>2. OnePendingPersistencePerNode'
      <3>1. RequestsUniqueByNode(AllPendingRequests)
        BY <1>1
           DEF StrongInductiveInvariant, Safety,
               OnePendingPersistencePerNode
      <3>2. AllPendingRequests' \subseteq AllPendingRequests
        BY <1>1, Isa DEF PersistProposal, AllPendingRequests
      <3> QED BY <3>1, <3>2,
                   RemovingRequestsPreservesNodeUniqueness
         DEF OnePendingPersistencePerNode
    <2>3. ProposalSigningRequiresIntent'
      BY <1>1, SMT
         DEF StrongInductiveInvariant, Safety, PersistProposal,
             ProposalSigningRequiresIntent, ProposalSign
    <2>4. /\ PrepareSigningRequiresIntent'
          /\ CommitSigningRequiresIntent'
          /\ TimeoutSigningRequiresIntent'
          /\ HonestPrepareUniqueness'
          /\ HonestCommitUniqueness'
          /\ HonestTimeoutUniqueness'
          /\ LockBelowHighest'
          /\ DecisionAgreement'
          /\ AppliedRequiresDecision'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, Safety, PersistProposal,
             PrepareSigningRequiresIntent,
             CommitSigningRequiresIntent, TimeoutSigningRequiresIntent,
             HonestPrepareUniqueness, HonestCommitUniqueness,
             HonestTimeoutUniqueness, LockBelowHighest, DecisionAgreement,
             AppliedRequiresDecision
    <2>5. Safety'
      BY <2>1, <2>2, <2>3, <2>4 DEF Safety
    <2>6. /\ ContextIdentityBindsFrozenEpoch'
          /\ OldContextCertificateRejected'
          /\ ContextParentWasApplied'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, PersistProposal,
             ContextIdentityBindsFrozenEpoch,
             OldContextCertificateRejected, ContextParentWasApplied,
             QcValid, CurrentEpoch
    <2>7. ReducerProvenanceInvariant'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, PersistProposal,
             ReducerProvenanceInvariant, HonestVoteUnique,
             HonestTimeoutUnique, IntentPhasesCorrect,
             PendingVoteWritesAuthorized,
             PendingCertificateWritesAuthorized,
             HonestVoteTransportBacked, QcTransportBacked,
             HonestTimeoutTransportBacked, TcTransportBacked,
             CertificatesBackedByIntents, HonestDurableIntentsSound,
             FormedTimeoutCertificatesSound,
             DurableTimeoutsProtectCommits, HighestAndLockAreCertified,
             VoteIntentFor
    <2> QED BY <2>5, <2>6, <2>7 DEF StrongInductiveInvariant
  <1> QED BY <1>1

THEOREM CompleteProposalSignaturePreservesStrongInvariant ==
  \A request:
    StrongInductiveInvariant /\ CompleteProposalSignature(request)
      => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME NEW request,
              StrongInductiveInvariant,
              CompleteProposalSignature(request)
         PROVE StrongInductiveInvariant'
    <2>1. TypeInvariant'
      BY <1>1, SMT
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             CompleteProposalSignature
    <2>2. Safety'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, Safety,
             CompleteProposalSignature,
             OnePendingPersistencePerNode, AllPendingRequests,
             RequestsUniqueByNode, ProposalSigningRequiresIntent,
             PrepareSigningRequiresIntent, CommitSigningRequiresIntent,
             TimeoutSigningRequiresIntent, HonestPrepareUniqueness,
             HonestCommitUniqueness, HonestTimeoutUniqueness,
             LockBelowHighest, DecisionAgreement, AppliedRequiresDecision,
             TypeInvariant
    <2>3. /\ ContextIdentityBindsFrozenEpoch'
          /\ OldContextCertificateRejected'
          /\ ContextParentWasApplied'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, CompleteProposalSignature,
             ContextIdentityBindsFrozenEpoch,
             OldContextCertificateRejected, ContextParentWasApplied,
             QcValid, CurrentEpoch
    <2>4. ReducerProvenanceInvariant'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, CompleteProposalSignature,
             ReducerProvenanceInvariant, HonestVoteUnique,
             HonestTimeoutUnique, IntentPhasesCorrect,
             PendingVoteWritesAuthorized,
             PendingCertificateWritesAuthorized,
             HonestVoteTransportBacked, QcTransportBacked,
             HonestTimeoutTransportBacked, TcTransportBacked,
             CertificatesBackedByIntents, HonestDurableIntentsSound,
             FormedTimeoutCertificatesSound,
             DurableTimeoutsProtectCommits, HighestAndLockAreCertified,
             VoteIntentFor
    <2> QED BY <2>2, <2>3, <2>4 DEF StrongInductiveInvariant
  <1> QED BY <1>1

THEOREM ResumeProposalPreservesStrongInvariant ==
  \A node, proposal:
    StrongInductiveInvariant /\ ResumeProposal(node, proposal)
      => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME NEW node,
              NEW proposal,
              StrongInductiveInvariant,
              ResumeProposal(node, proposal)
         PROVE StrongInductiveInvariant'
    <2>1. TypeInvariant'
      BY <1>1, SMT
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             ResumeProposal, ProposalSign, ProposalSignSet
    <2>2. ProposalSigningRequiresIntent'
      BY <1>1, SMT
         DEF StrongInductiveInvariant, Safety, ResumeProposal,
             ProposalSigningRequiresIntent, ProposalSign
    <2>3. Safety'
      BY <1>1, <2>1, <2>2, Isa
         DEF StrongInductiveInvariant, Safety, ResumeProposal,
             OnePendingPersistencePerNode, AllPendingRequests,
             RequestsUniqueByNode, PrepareSigningRequiresIntent,
             CommitSigningRequiresIntent, TimeoutSigningRequiresIntent,
             HonestPrepareUniqueness, HonestCommitUniqueness,
             HonestTimeoutUniqueness, LockBelowHighest, DecisionAgreement,
             AppliedRequiresDecision
    <2>4. /\ ContextIdentityBindsFrozenEpoch'
          /\ OldContextCertificateRejected'
          /\ ContextParentWasApplied'
          /\ ReducerProvenanceInvariant'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, ResumeProposal,
             ContextIdentityBindsFrozenEpoch,
             OldContextCertificateRejected, ContextParentWasApplied,
             QcValid, CurrentEpoch, ReducerProvenanceInvariant,
             HonestVoteUnique, HonestTimeoutUnique, IntentPhasesCorrect,
             PendingVoteWritesAuthorized,
             PendingCertificateWritesAuthorized,
             HonestVoteTransportBacked, QcTransportBacked,
             HonestTimeoutTransportBacked, TcTransportBacked,
             CertificatesBackedByIntents, HonestDurableIntentsSound,
             FormedTimeoutCertificatesSound,
             DurableTimeoutsProtectCommits, HighestAndLockAreCertified,
             VoteIntentFor
    <2> QED BY <2>3, <2>4 DEF StrongInductiveInvariant
  <1> QED BY <1>1

THEOREM BeginPreparePreservesStrongInvariant ==
  \A node, proposal:
    StrongInductiveInvariant /\ BeginPrepare(node, proposal)
      => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME NEW node,
              NEW proposal,
              StrongInductiveInvariant,
              BeginPrepare(node, proposal)
         PROVE StrongInductiveInvariant'
    <2>1. TypeInvariant'
      BY <1>1, SMT
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             BeginPrepare
    <2>2. OnePendingPersistencePerNode'
      <3>1. RequestsUniqueByNode(AllPendingRequests)
        BY <1>1
           DEF StrongInductiveInvariant, Safety,
               OnePendingPersistencePerNode
      <3>2. node \notin RequestNodeSet(AllPendingRequests)
        BY <1>1, PendingNodesAreAllRequestNodes
           DEF BeginPrepare, NodeIdle
      <3>3. AllPendingRequests'
               = AllPendingRequests
                   \cup {PrepareRequestFor(node, proposal)}
        BY <1>1, Isa DEF BeginPrepare, AllPendingRequests
      <3>4. PrepareRequestFor(node, proposal).node = node
        BY DEF PrepareRequestFor, PrepareWal
      <3>5. RequestsUniqueByNode(
               AllPendingRequests
                 \cup {PrepareRequestFor(node, proposal)})
        BY <3>1, <3>2, <3>4,
           NewRequestPreservesNodeUniqueness
      <3> QED BY <3>3, <3>5
         DEF OnePendingPersistencePerNode
    <2>3. /\ ProposalSigningRequiresIntent'
          /\ PrepareSigningRequiresIntent'
          /\ CommitSigningRequiresIntent'
          /\ TimeoutSigningRequiresIntent'
          /\ HonestPrepareUniqueness'
          /\ HonestCommitUniqueness'
          /\ HonestTimeoutUniqueness'
          /\ LockBelowHighest'
          /\ DecisionAgreement'
          /\ AppliedRequiresDecision'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, Safety, BeginPrepare,
             ProposalSigningRequiresIntent, PrepareSigningRequiresIntent,
             CommitSigningRequiresIntent, TimeoutSigningRequiresIntent,
             HonestPrepareUniqueness, HonestCommitUniqueness,
             HonestTimeoutUniqueness, LockBelowHighest, DecisionAgreement,
             AppliedRequiresDecision
    <2>4. Safety'
      BY <2>1, <2>2, <2>3 DEF Safety
    <2>5. /\ ContextIdentityBindsFrozenEpoch'
          /\ OldContextCertificateRejected'
          /\ ContextParentWasApplied'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, BeginPrepare,
             ContextIdentityBindsFrozenEpoch,
             OldContextCertificateRejected, ContextParentWasApplied,
             QcValid, CurrentEpoch
    <2>6. PendingVoteWritesAuthorized'
      BY <1>1, SMT
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             BeginPrepare, PendingVoteWritesAuthorized,
             PrepareRequestFor, PrepareVoteFor, PrepareWal, Vote,
             CanAppendVote, SameVoteSlot
      <2>7. /\ HonestVoteUnique(prepareIntents)'
          /\ HonestVoteUnique(commitIntents)'
          /\ HonestTimeoutUnique(timeoutIntents)'
          /\ IntentPhasesCorrect'
          /\ PendingCertificateWritesAuthorized'
          /\ HonestVoteTransportBacked'
          /\ QcTransportBacked'
          /\ HonestTimeoutTransportBacked'
          /\ TcTransportBacked'
          /\ CertificatesBackedByIntents'
          /\ HonestDurableIntentsSound'
          /\ FormedTimeoutCertificatesSound'
          /\ DurableTimeoutsProtectCommits'
          /\ HighestAndLockAreCertified'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             BeginPrepare, HonestVoteUnique, HonestTimeoutUnique,
             IntentPhasesCorrect,
             PendingCertificateWritesAuthorized,
             HonestVoteTransportBacked, QcTransportBacked,
             HonestTimeoutTransportBacked, TcTransportBacked,
             CertificatesBackedByIntents, HonestDurableIntentsSound,
             FormedTimeoutCertificatesSound,
             DurableTimeoutsProtectCommits, HighestAndLockAreCertified,
             VoteIntentFor
    <2>8. ReducerProvenanceInvariant'
      BY <2>6, <2>7 DEF ReducerProvenanceInvariant
    <2> QED BY <2>4, <2>5, <2>8 DEF StrongInductiveInvariant
  <1> QED BY <1>1

THEOREM HonestIntentSoundAppend ==
  \A intents, vote, durable, validSubjects:
    HonestIntentSound(intents, durable, validSubjects)
      /\ (vote.signer \in Honest
            => /\ vote.subject \in validSubjects
               /\ BodyHeldBy(durable, vote.signer,
                             vote.context, vote.subject))
      => HonestIntentSound(intents \cup {vote}, durable, validSubjects)
BY SMT DEF HonestIntentSound

THEOREM CertificateBackingIsMonotone ==
  \A epoch, qc, before, after:
    CertificateBackedBy(epoch, qc, before)
      /\ before \subseteq after
      => CertificateBackedBy(epoch, qc, after)
BY DEF CertificateBackedBy

THEOREM PhasedVoteUniquenessImpliesSlotUniqueness ==
  \A intents, phase:
    HonestVoteUnique(intents)
      /\ (\A vote \in intents: vote.phase = phase)
      => \A left, right \in intents:
           (left.signer \in Honest
             /\ right.signer = left.signer
             /\ right.context = left.context
             /\ right.view = left.view)
           => right.subject = left.subject
BY SMT DEF HonestVoteUnique, SameVoteSlot

THEOREM PersistPreparePreservesStrongInvariant ==
  \A request:
    StrongInductiveInvariant /\ PersistPrepare(request)
      => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME NEW request,
              StrongInductiveInvariant,
              PersistPrepare(request)
         PROVE StrongInductiveInvariant'
    <2>1. /\ request.node \in ValidatorIds
          /\ request.vote \in VoteRecordSet
          /\ request.vote.phase = "Prepare"
          /\ request.vote.signer = request.node
          /\ request.node \in Honest
          /\ request.vote.subject \in ValidSubjects
          /\ BodyHeldBy(durableBodies, request.node,
                        request.vote.context, request.vote.subject)
          /\ CanAppendVote(prepareIntents, request.vote)
      BY <1>1
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             ReducerProvenanceInvariant, PendingVoteWritesAuthorized,
             PersistPrepare, PrepareWalSet
    <2>2. TypeInvariant'
      <3>1. /\ prepareIntents' \subseteq VoteRecordSet
            /\ pendingPrepare' \subseteq PrepareWalSet
        BY <1>1, <2>1, Isa
           DEF StrongInductiveInvariant, Safety, TypeInvariant,
               PersistPrepare
      <3>2. VoteSign(request.node, request.vote) \in VoteSignSet
        BY <2>1 DEF VoteSign, VoteSignSet
      <3>3. signVotes' \subseteq VoteSignSet
        BY <1>1, <3>2, Isa
           DEF StrongInductiveInvariant, Safety, TypeInvariant,
               PersistPrepare
      <3> QED BY <1>1, <3>1, <3>3, Isa
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             PersistPrepare
    <2>3. OnePendingPersistencePerNode'
      <3>1. RequestsUniqueByNode(AllPendingRequests)
        BY <1>1
           DEF StrongInductiveInvariant, Safety,
               OnePendingPersistencePerNode
      <3>2. AllPendingRequests' \subseteq AllPendingRequests
        BY <1>1, Isa DEF PersistPrepare, AllPendingRequests
      <3> QED BY <3>1, <3>2,
                   RemovingRequestsPreservesNodeUniqueness
         DEF OnePendingPersistencePerNode
    <2>4. HonestVoteUnique(prepareIntents)'
      BY <1>1, <2>1, DurableVoteAppendPreservesUniqueness
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             PersistPrepare
    <2>5. IntentPhasesCorrect'
      BY <1>1, <2>1, SMT
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             IntentPhasesCorrect, PersistPrepare
    <2>6. HonestDurableIntentsSound'
      BY <1>1, <2>1, HonestIntentSoundAppend
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             HonestDurableIntentsSound, PersistPrepare
    <2>7. PrepareSigningRequiresIntent'
      BY <1>1, SMT
         DEF StrongInductiveInvariant, Safety, PersistPrepare,
             PrepareSigningRequiresIntent, VoteSign
    <2>8. HonestPrepareUniqueness'
      BY <2>4, <2>5, PhasedVoteUniquenessImpliesSlotUniqueness
         DEF HonestPrepareUniqueness, IntentPhasesCorrect
    <2>9. CommitSigningRequiresIntent'
      BY <1>1, <2>1, SMT
         DEF StrongInductiveInvariant, Safety, PersistPrepare,
             CommitSigningRequiresIntent, VoteSign
    <2>10. /\ ProposalSigningRequiresIntent'
          /\ TimeoutSigningRequiresIntent'
          /\ HonestCommitUniqueness'
          /\ HonestTimeoutUniqueness'
          /\ LockBelowHighest'
          /\ DecisionAgreement'
          /\ AppliedRequiresDecision'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, Safety, PersistPrepare,
             ProposalSigningRequiresIntent, TimeoutSigningRequiresIntent,
             HonestCommitUniqueness,
             HonestTimeoutUniqueness, LockBelowHighest, DecisionAgreement,
             AppliedRequiresDecision
    <2>11. Safety'
      BY <2>2, <2>3, <2>7, <2>8, <2>9, <2>10 DEF Safety
    <2>12. /\ ContextIdentityBindsFrozenEpoch'
           /\ OldContextCertificateRejected'
           /\ ContextParentWasApplied'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, PersistPrepare,
             ContextIdentityBindsFrozenEpoch,
             OldContextCertificateRejected, ContextParentWasApplied,
             QcValid, CurrentEpoch
    <2>13. PendingVoteWritesAuthorized'
      BY <1>1, <2>1, SMT
         DEF StrongInductiveInvariant, Safety, OnePendingPersistencePerNode,
             AllPendingRequests, RequestsUniqueByNode,
             ReducerProvenanceInvariant, PendingVoteWritesAuthorized,
             PersistPrepare, CanAppendVote, SameVoteSlot
    <2>14. /\ HonestVoteUnique(commitIntents)'
           /\ HonestTimeoutUnique(timeoutIntents)'
           /\ PendingCertificateWritesAuthorized'
           /\ QcTransportBacked'
           /\ HonestTimeoutTransportBacked'
           /\ TcTransportBacked'
           /\ FormedTimeoutCertificatesSound'
           /\ DurableTimeoutsProtectCommits'
           /\ HighestAndLockAreCertified'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             PersistPrepare, HonestVoteUnique, HonestTimeoutUnique,
             PendingCertificateWritesAuthorized, QcTransportBacked,
             HonestTimeoutTransportBacked, TcTransportBacked,
             FormedTimeoutCertificatesSound,
             DurableTimeoutsProtectCommits, HighestAndLockAreCertified
    <2>15. HonestVoteTransportBacked'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             PersistPrepare, HonestVoteTransportBacked, VoteIntentFor,
             IntentPhasesCorrect
    <2>16. CertificatesBackedByIntents'
      <3>1. prepareIntents \subseteq prepareIntents'
        BY <1>1, Isa DEF PersistPrepare
      <3>2. \A qc \in prepareQCs:
               /\ HistoricalQcValid(qc)
               /\ CertificateBackedBy(qc.context.epoch, qc,
                                      prepareIntents')
        BY <1>1, <3>1, CertificateBackingIsMonotone
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               CertificatesBackedByIntents, PersistPrepare
      <3>3. \A qc \in commitQCs:
               /\ HistoricalQcValid(qc)
               /\ CertificateBackedBy(qc.context.epoch, qc,
                                      commitIntents')
        BY <1>1, Isa
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               CertificatesBackedByIntents, PersistPrepare
      <3>4. /\ prepareQCs' = prepareQCs
            /\ commitQCs' = commitQCs
        BY <1>1 DEF PersistPrepare
      <3> QED BY <3>2, <3>3, <3>4
         DEF CertificatesBackedByIntents
    <2>17. ReducerProvenanceInvariant'
      BY <2>4, <2>5, <2>6, <2>13, <2>14, <2>15, <2>16
         DEF ReducerProvenanceInvariant
    <2> QED BY <2>11, <2>12, <2>17
       DEF StrongInductiveInvariant
  <1> QED BY <1>1

THEOREM CompleteVoteSignaturePreservesStrongInvariant ==
  \A request:
    StrongInductiveInvariant /\ CompleteVoteSignature(request)
      => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME NEW request,
              StrongInductiveInvariant,
              CompleteVoteSignature(request)
         PROVE StrongInductiveInvariant'
    <2>1. TypeInvariant'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             CompleteVoteSignature
    <2>2. /\ ProposalSigningRequiresIntent'
          /\ PrepareSigningRequiresIntent'
          /\ CommitSigningRequiresIntent'
          /\ TimeoutSigningRequiresIntent'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, Safety, CompleteVoteSignature,
             ProposalSigningRequiresIntent, PrepareSigningRequiresIntent,
             CommitSigningRequiresIntent, TimeoutSigningRequiresIntent
    <2>3. /\ OnePendingPersistencePerNode'
          /\ HonestPrepareUniqueness'
          /\ HonestCommitUniqueness'
          /\ HonestTimeoutUniqueness'
          /\ LockBelowHighest'
          /\ DecisionAgreement'
          /\ AppliedRequiresDecision'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, Safety, CompleteVoteSignature,
             OnePendingPersistencePerNode, AllPendingRequests,
             RequestsUniqueByNode, HonestPrepareUniqueness,
             HonestCommitUniqueness, HonestTimeoutUniqueness,
             LockBelowHighest, DecisionAgreement, AppliedRequiresDecision
    <2>4. Safety'
      BY <2>1, <2>2, <2>3 DEF Safety
    <2>5. /\ ContextIdentityBindsFrozenEpoch'
          /\ OldContextCertificateRejected'
          /\ ContextParentWasApplied'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, CompleteVoteSignature,
             ContextIdentityBindsFrozenEpoch,
             OldContextCertificateRejected, ContextParentWasApplied,
             QcValid, CurrentEpoch
    <2>6. HonestVoteTransportBacked'
      BY <1>1, SMT
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             CompleteVoteSignature, HonestVoteTransportBacked,
             VoteIntentFor, BroadcastVotes, VoteEnvelope,
             IntentPhasesCorrect
    <2>7. /\ HonestVoteUnique(prepareIntents)'
          /\ HonestVoteUnique(commitIntents)'
          /\ HonestTimeoutUnique(timeoutIntents)'
          /\ IntentPhasesCorrect'
          /\ PendingVoteWritesAuthorized'
          /\ PendingCertificateWritesAuthorized'
          /\ QcTransportBacked'
          /\ HonestTimeoutTransportBacked'
          /\ TcTransportBacked'
          /\ CertificatesBackedByIntents'
          /\ HonestDurableIntentsSound'
          /\ FormedTimeoutCertificatesSound'
          /\ DurableTimeoutsProtectCommits'
          /\ HighestAndLockAreCertified'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             CompleteVoteSignature, HonestVoteUnique,
             HonestTimeoutUnique, IntentPhasesCorrect,
             PendingVoteWritesAuthorized,
             PendingCertificateWritesAuthorized, QcTransportBacked,
             HonestTimeoutTransportBacked, TcTransportBacked,
             CertificatesBackedByIntents, HonestDurableIntentsSound,
             FormedTimeoutCertificatesSound,
             DurableTimeoutsProtectCommits, HighestAndLockAreCertified
    <2>8. ReducerProvenanceInvariant'
      BY <2>6, <2>7 DEF ReducerProvenanceInvariant
    <2> QED BY <2>4, <2>5, <2>8 DEF StrongInductiveInvariant
  <1> QED BY <1>1

THEOREM ByzantineBroadcastVotePreservesStrongInvariant ==
  \A signer, roundView, phase, subject:
    StrongInductiveInvariant
      /\ ByzantineBroadcastVote(signer, roundView, phase, subject)
      => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME NEW signer,
              NEW roundView,
              NEW phase,
              NEW subject,
              StrongInductiveInvariant,
              ByzantineBroadcastVote(signer, roundView, phase, subject)
         PROVE StrongInductiveInvariant'
    <2>1. signer \notin Honest
      BY <1>1 DEF ByzantineBroadcastVote, Byzantine
    <2>2. HonestVoteTransportBacked'
      <3>1. \A envelope \in voteNetwork:
               envelope.vote.signer \in Honest
                 => VoteIntentFor(envelope.vote)
        BY <1>1
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               HonestVoteTransportBacked
      <3>2. \A envelope \in
                    BroadcastVotes(
                      Vote(context, roundView, phase, subject, signer)):
               envelope.vote.signer \notin Honest
        BY <2>1 DEF BroadcastVotes, VoteEnvelope, Vote
      <3>3. \A envelope \in voteNetwork':
               envelope.vote.signer \in Honest
                 => VoteIntentFor(envelope.vote)'
        BY <1>1, <3>1, <3>2, SMT DEF ByzantineBroadcastVote
      <3>4. \A received \in receivedVotes':
               received.vote.signer \in Honest
                 => VoteIntentFor(received.vote)'
        BY <1>1, Isa
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               ByzantineBroadcastVote, HonestVoteTransportBacked,
               VoteIntentFor
      <3> QED BY <3>3, <3>4 DEF HonestVoteTransportBacked
    <2>3. /\ Safety'
          /\ ContextIdentityBindsFrozenEpoch'
          /\ OldContextCertificateRejected'
          /\ ContextParentWasApplied'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, Safety, ByzantineBroadcastVote,
             TypeInvariant, OnePendingPersistencePerNode,
             AllPendingRequests, RequestsUniqueByNode,
             ProposalSigningRequiresIntent, PrepareSigningRequiresIntent,
             CommitSigningRequiresIntent, TimeoutSigningRequiresIntent,
             HonestPrepareUniqueness, HonestCommitUniqueness,
             HonestTimeoutUniqueness, LockBelowHighest, DecisionAgreement,
             AppliedRequiresDecision, ContextIdentityBindsFrozenEpoch,
             OldContextCertificateRejected, ContextParentWasApplied,
             QcValid, CurrentEpoch
    <2>4. /\ HonestVoteUnique(prepareIntents)'
          /\ HonestVoteUnique(commitIntents)'
          /\ HonestTimeoutUnique(timeoutIntents)'
          /\ IntentPhasesCorrect'
          /\ PendingVoteWritesAuthorized'
          /\ PendingCertificateWritesAuthorized'
          /\ QcTransportBacked'
          /\ HonestTimeoutTransportBacked'
          /\ TcTransportBacked'
          /\ CertificatesBackedByIntents'
          /\ HonestDurableIntentsSound'
          /\ FormedTimeoutCertificatesSound'
          /\ DurableTimeoutsProtectCommits'
          /\ HighestAndLockAreCertified'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             ByzantineBroadcastVote, HonestVoteUnique,
             HonestTimeoutUnique, IntentPhasesCorrect,
             PendingVoteWritesAuthorized,
             PendingCertificateWritesAuthorized, QcTransportBacked,
             HonestTimeoutTransportBacked, TcTransportBacked,
             CertificatesBackedByIntents, HonestDurableIntentsSound,
             FormedTimeoutCertificatesSound,
             DurableTimeoutsProtectCommits, HighestAndLockAreCertified
    <2>5. ReducerProvenanceInvariant'
      BY <2>2, <2>4 DEF ReducerProvenanceInvariant
    <2> QED BY <2>3, <2>5 DEF StrongInductiveInvariant
  <1> QED BY <1>1

THEOREM DeliverVotePreservesStrongInvariant ==
  \A envelope:
    StrongInductiveInvariant /\ DeliverVote(envelope)
      => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME NEW envelope,
              StrongInductiveInvariant,
              DeliverVote(envelope)
         PROVE StrongInductiveInvariant'
    <2>1. HonestVoteTransportBacked'
      BY <1>1, SMT
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             DeliverVote, HonestVoteTransportBacked,
             VoteIntentFor, VoteAt
    <2>2. /\ Safety'
          /\ ContextIdentityBindsFrozenEpoch'
          /\ OldContextCertificateRejected'
          /\ ContextParentWasApplied'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, Safety, DeliverVote,
             TypeInvariant, OnePendingPersistencePerNode,
             AllPendingRequests, RequestsUniqueByNode,
             ProposalSigningRequiresIntent, PrepareSigningRequiresIntent,
             CommitSigningRequiresIntent, TimeoutSigningRequiresIntent,
             HonestPrepareUniqueness, HonestCommitUniqueness,
             HonestTimeoutUniqueness, LockBelowHighest, DecisionAgreement,
             AppliedRequiresDecision, ContextIdentityBindsFrozenEpoch,
             OldContextCertificateRejected, ContextParentWasApplied,
             QcValid, CurrentEpoch
    <2>3. /\ HonestVoteUnique(prepareIntents)'
          /\ HonestVoteUnique(commitIntents)'
          /\ HonestTimeoutUnique(timeoutIntents)'
          /\ IntentPhasesCorrect'
          /\ PendingVoteWritesAuthorized'
          /\ PendingCertificateWritesAuthorized'
          /\ QcTransportBacked'
          /\ HonestTimeoutTransportBacked'
          /\ TcTransportBacked'
          /\ CertificatesBackedByIntents'
          /\ HonestDurableIntentsSound'
          /\ FormedTimeoutCertificatesSound'
          /\ DurableTimeoutsProtectCommits'
          /\ HighestAndLockAreCertified'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             DeliverVote, HonestVoteUnique, HonestTimeoutUnique,
             IntentPhasesCorrect, PendingVoteWritesAuthorized,
             PendingCertificateWritesAuthorized, QcTransportBacked,
             HonestTimeoutTransportBacked, TcTransportBacked,
             CertificatesBackedByIntents, HonestDurableIntentsSound,
             FormedTimeoutCertificatesSound,
             DurableTimeoutsProtectCommits, HighestAndLockAreCertified
    <2>4. ReducerProvenanceInvariant'
      BY <2>1, <2>3 DEF ReducerProvenanceInvariant
    <2> QED BY <2>2, <2>4 DEF StrongInductiveInvariant
  <1> QED BY <1>1

THEOREM ResumeVotePreservesStrongInvariant ==
  \A node, vote:
    StrongInductiveInvariant /\ ResumeVote(node, vote)
      => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME NEW node,
              NEW vote,
              StrongInductiveInvariant,
              ResumeVote(node, vote)
         PROVE StrongInductiveInvariant'
    <2>1. TypeInvariant'
      BY <1>1, SMT
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             ResumeVote, VoteSign, VoteSignSet
    <2>2. /\ PrepareSigningRequiresIntent'
          /\ CommitSigningRequiresIntent'
      BY <1>1, SMT
         DEF StrongInductiveInvariant, Safety, ResumeVote,
             PrepareSigningRequiresIntent, CommitSigningRequiresIntent,
             VoteSign, IntentPhasesCorrect, ReducerProvenanceInvariant
    <2>3. /\ Safety'
          /\ ContextIdentityBindsFrozenEpoch'
          /\ OldContextCertificateRejected'
          /\ ContextParentWasApplied'
          /\ ReducerProvenanceInvariant'
      BY <1>1, <2>1, <2>2, Isa
         DEF StrongInductiveInvariant, Safety, ResumeVote,
             TypeInvariant, OnePendingPersistencePerNode,
             AllPendingRequests, RequestsUniqueByNode,
             ProposalSigningRequiresIntent, TimeoutSigningRequiresIntent,
             HonestPrepareUniqueness, HonestCommitUniqueness,
             HonestTimeoutUniqueness, LockBelowHighest, DecisionAgreement,
             AppliedRequiresDecision, ContextIdentityBindsFrozenEpoch,
             OldContextCertificateRejected, ContextParentWasApplied,
             QcValid, CurrentEpoch, ReducerProvenanceInvariant,
             HonestVoteUnique, HonestTimeoutUnique, IntentPhasesCorrect,
             PendingVoteWritesAuthorized,
             PendingCertificateWritesAuthorized,
             HonestVoteTransportBacked, QcTransportBacked,
             HonestTimeoutTransportBacked, TcTransportBacked,
             CertificatesBackedByIntents, HonestDurableIntentsSound,
             FormedTimeoutCertificatesSound,
             DurableTimeoutsProtectCommits, HighestAndLockAreCertified,
             VoteIntentFor
    <2> QED BY <2>3 DEF StrongInductiveInvariant
  <1> QED BY <1>1

THEOREM CurrentQcValidityIsHistorical ==
  \A qc:
    TypeInvariant /\ QcValid(qc) => HistoricalQcValid(qc)
PROOF
  <1>1. ASSUME NEW qc,
              TypeInvariant,
              QcValid(qc)
         PROVE HistoricalQcValid(qc)
    <2>1. /\ qc.context \in ContextRecords
          /\ qc.height = qc.context.height
          /\ qc.view \in Views
          /\ qc.phase \in Phases
          /\ qc.subject \in ValidSubjects
          /\ DualQuorum(qc.context.epoch, qc.signers)
      BY <1>1 DEF TypeInvariant, QcValid, CurrentEpoch
    <2>2. qc.context.epoch \in Epochs
      BY <1>1
         DEF QcValid, CurrentEpoch, DualQuorum, CountQuorum
    <2> QED BY <2>1, <2>2 DEF HistoricalQcValid
  <1> QED BY <1>1

THEOREM CurrentQcBackingIsCertificateBacking ==
  \A qc, intents:
    QcValid(qc) /\ CertificateHonestIntentBacked(qc, intents)
      => CertificateBackedBy(CurrentEpoch, qc, intents)
BY DEF QcValid, CertificateHonestIntentBacked, CertificateBackedBy

THEOREM FormPrepareQCPreservesStrongInvariant ==
  \A node, roundView, subject:
    StrongInductiveInvariant /\ FormPrepareQC(node, roundView, subject)
      => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME NEW node,
              NEW roundView,
              NEW subject,
              StrongInductiveInvariant,
              FormPrepareQC(node, roundView, subject)
         PROVE StrongInductiveInvariant'
    <2> DEFINE NewQc ==
           QC(context, roundView, "Prepare", subject,
              VoteSignersAt(node, roundView, "Prepare", subject))
    <2>1. /\ QcValid(NewQc)
          /\ NewQc \in QcRecordSet
          /\ CertificateHonestIntentBacked(NewQc, prepareIntents)
      BY <1>1 DEF FormPrepareQC, NewQc
    <2>2. /\ HistoricalQcValid(NewQc)
          /\ CertificateBackedBy(CurrentEpoch, NewQc, prepareIntents)
      BY <1>1, <2>1, CurrentQcValidityIsHistorical,
         CurrentQcBackingIsCertificateBacking
         DEF StrongInductiveInvariant, Safety
    <2>3. TypeInvariant'
      BY <1>1, <2>1, Isa
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             FormPrepareQC, NewQc
    <2>4. /\ OnePendingPersistencePerNode'
          /\ ProposalSigningRequiresIntent'
          /\ PrepareSigningRequiresIntent'
          /\ CommitSigningRequiresIntent'
          /\ TimeoutSigningRequiresIntent'
          /\ HonestPrepareUniqueness'
          /\ HonestCommitUniqueness'
          /\ HonestTimeoutUniqueness'
          /\ LockBelowHighest'
          /\ DecisionAgreement'
          /\ AppliedRequiresDecision'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, Safety, FormPrepareQC,
             OnePendingPersistencePerNode, AllPendingRequests,
             RequestsUniqueByNode, ProposalSigningRequiresIntent,
             PrepareSigningRequiresIntent, CommitSigningRequiresIntent,
             TimeoutSigningRequiresIntent, HonestPrepareUniqueness,
             HonestCommitUniqueness, HonestTimeoutUniqueness,
             LockBelowHighest, DecisionAgreement, AppliedRequiresDecision
    <2>5. Safety'
      BY <2>3, <2>4 DEF Safety
    <2>6. /\ ContextIdentityBindsFrozenEpoch'
          /\ OldContextCertificateRejected'
          /\ ContextParentWasApplied'
      BY <1>1, <2>1, Isa
         DEF StrongInductiveInvariant, FormPrepareQC, NewQc,
             ContextIdentityBindsFrozenEpoch,
             OldContextCertificateRejected, ContextParentWasApplied,
             QcValid, CurrentEpoch
    <2>7. CertificatesBackedByIntents'
      <3>1. /\ prepareQCs' = prepareQCs \cup {NewQc}
            /\ prepareIntents' = prepareIntents
        BY <1>1 DEF FormPrepareQC, NewQc
      <3>2. \A qc \in prepareQCs:
               /\ HistoricalQcValid(qc)
               /\ CertificateBackedBy(qc.context.epoch, qc,
                                      prepareIntents)
        BY <1>1
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               CertificatesBackedByIntents
      <3>3. \A qc \in prepareQCs':
               /\ HistoricalQcValid(qc)
               /\ CertificateBackedBy(qc.context.epoch, qc,
                                      prepareIntents')
        BY <1>1, <2>2, <3>1, <3>2, SMT DEF CurrentEpoch
      <3>4. \A qc \in commitQCs':
               /\ HistoricalQcValid(qc)
               /\ CertificateBackedBy(qc.context.epoch, qc,
                                      commitIntents')
        BY <1>1, Isa
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               CertificatesBackedByIntents, FormPrepareQC
      <3> QED BY <3>3, <3>4 DEF CertificatesBackedByIntents
    <2>8. QcTransportBacked'
      BY <1>1, SMT
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             FormPrepareQC, NewQc, QcTransportBacked,
             BroadcastQCs, QcEnvelope
    <2>9. PendingCertificateWritesAuthorized'
      BY <1>1, SMT
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             FormPrepareQC, NewQc, PendingCertificateWritesAuthorized
    <2>10. HighestAndLockAreCertified'
      BY <1>1, SMT
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             FormPrepareQC, NewQc, HighestAndLockAreCertified
    <2>11. /\ HonestVoteUnique(prepareIntents)'
          /\ HonestVoteUnique(commitIntents)'
          /\ HonestTimeoutUnique(timeoutIntents)'
          /\ IntentPhasesCorrect'
          /\ PendingVoteWritesAuthorized'
          /\ HonestVoteTransportBacked'
          /\ HonestTimeoutTransportBacked'
          /\ TcTransportBacked'
          /\ HonestDurableIntentsSound'
          /\ FormedTimeoutCertificatesSound'
          /\ DurableTimeoutsProtectCommits'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             FormPrepareQC, HonestVoteUnique, HonestTimeoutUnique,
             IntentPhasesCorrect, PendingVoteWritesAuthorized,
             HonestVoteTransportBacked, HonestTimeoutTransportBacked,
             TcTransportBacked, HonestDurableIntentsSound,
             FormedTimeoutCertificatesSound,
             DurableTimeoutsProtectCommits
    <2>12. ReducerProvenanceInvariant'
      BY <2>7, <2>8, <2>9, <2>10, <2>11
         DEF ReducerProvenanceInvariant
    <2> QED BY <2>5, <2>6, <2>12 DEF StrongInductiveInvariant
  <1> QED BY <1>1

THEOREM DeliverQCPreservesStrongInvariant ==
  \A envelope:
    StrongInductiveInvariant /\ DeliverQC(envelope)
      => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME NEW envelope,
              StrongInductiveInvariant,
              DeliverQC(envelope)
         PROVE StrongInductiveInvariant'
    <2>1. QcTransportBacked'
      BY <1>1, SMT
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             DeliverQC, QcTransportBacked, QcAt
    <2>2. /\ Safety'
          /\ ContextIdentityBindsFrozenEpoch'
          /\ OldContextCertificateRejected'
          /\ ContextParentWasApplied'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, Safety, DeliverQC,
             TypeInvariant, OnePendingPersistencePerNode,
             AllPendingRequests, RequestsUniqueByNode,
             ProposalSigningRequiresIntent, PrepareSigningRequiresIntent,
             CommitSigningRequiresIntent, TimeoutSigningRequiresIntent,
             HonestPrepareUniqueness, HonestCommitUniqueness,
             HonestTimeoutUniqueness, LockBelowHighest, DecisionAgreement,
             AppliedRequiresDecision, ContextIdentityBindsFrozenEpoch,
             OldContextCertificateRejected, ContextParentWasApplied,
             QcValid, CurrentEpoch
    <2>3. /\ HonestVoteUnique(prepareIntents)'
          /\ HonestVoteUnique(commitIntents)'
          /\ HonestTimeoutUnique(timeoutIntents)'
          /\ IntentPhasesCorrect'
          /\ PendingVoteWritesAuthorized'
          /\ PendingCertificateWritesAuthorized'
          /\ HonestVoteTransportBacked'
          /\ HonestTimeoutTransportBacked'
          /\ TcTransportBacked'
          /\ CertificatesBackedByIntents'
          /\ HonestDurableIntentsSound'
          /\ FormedTimeoutCertificatesSound'
          /\ DurableTimeoutsProtectCommits'
          /\ HighestAndLockAreCertified'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             DeliverQC, HonestVoteUnique, HonestTimeoutUnique,
             IntentPhasesCorrect, PendingVoteWritesAuthorized,
             PendingCertificateWritesAuthorized,
             HonestVoteTransportBacked, HonestTimeoutTransportBacked,
             TcTransportBacked, CertificatesBackedByIntents,
             HonestDurableIntentsSound, FormedTimeoutCertificatesSound,
             DurableTimeoutsProtectCommits, HighestAndLockAreCertified
    <2>4. ReducerProvenanceInvariant'
      BY <2>1, <2>3 DEF ReducerProvenanceInvariant
    <2> QED BY <2>2, <2>4 DEF StrongInductiveInvariant
  <1> QED BY <1>1

THEOREM RestartPreservesStrongInvariant ==
  \A node:
    StrongInductiveInvariant /\ Restart(node)
      => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME NEW node,
              StrongInductiveInvariant,
              Restart(node)
         PROVE StrongInductiveInvariant'
    <2>1. TypeInvariant'
      BY <1>1, SMT
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             Restart, Generations
    <2>2. /\ OnePendingPersistencePerNode'
          /\ ProposalSigningRequiresIntent'
          /\ PrepareSigningRequiresIntent'
          /\ CommitSigningRequiresIntent'
          /\ TimeoutSigningRequiresIntent'
          /\ HonestPrepareUniqueness'
          /\ HonestCommitUniqueness'
          /\ HonestTimeoutUniqueness'
          /\ LockBelowHighest'
          /\ DecisionAgreement'
          /\ AppliedRequiresDecision'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, Safety, Restart,
             OnePendingPersistencePerNode, AllPendingRequests,
             RequestsUniqueByNode, ProposalSigningRequiresIntent,
             PrepareSigningRequiresIntent, CommitSigningRequiresIntent,
             TimeoutSigningRequiresIntent, HonestPrepareUniqueness,
             HonestCommitUniqueness, HonestTimeoutUniqueness,
             LockBelowHighest, DecisionAgreement, AppliedRequiresDecision
    <2>3. Safety'
      BY <2>1, <2>2 DEF Safety
    <2>4. /\ ContextIdentityBindsFrozenEpoch'
          /\ OldContextCertificateRejected'
          /\ ContextParentWasApplied'
          /\ ReducerProvenanceInvariant'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, Restart,
             ContextIdentityBindsFrozenEpoch,
             OldContextCertificateRejected, ContextParentWasApplied,
             QcValid, CurrentEpoch, ReducerProvenanceInvariant,
             HonestVoteUnique, HonestTimeoutUnique, IntentPhasesCorrect,
             PendingVoteWritesAuthorized,
             PendingCertificateWritesAuthorized,
             HonestVoteTransportBacked, QcTransportBacked,
             HonestTimeoutTransportBacked, TcTransportBacked,
             CertificatesBackedByIntents, HonestDurableIntentsSound,
             FormedTimeoutCertificatesSound,
             DurableTimeoutsProtectCommits, HighestAndLockAreCertified,
             VoteIntentFor
    <2> QED BY <2>3, <2>4 DEF StrongInductiveInvariant
  <1> QED BY <1>1

THEOREM CrashPreservesStrongInvariant ==
  \A node:
    StrongInductiveInvariant /\ Crash(node)
      => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME NEW node,
              StrongInductiveInvariant,
              Crash(node)
         PROVE StrongInductiveInvariant'
    <2>1. TypeInvariant'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, Safety, TypeInvariant, Crash
    <2>2. OnePendingPersistencePerNode'
      <3>1. AllPendingRequests' \subseteq AllPendingRequests
        BY <1>1, Isa DEF Crash, AllPendingRequests
      <3>2. RequestsUniqueByNode(AllPendingRequests)
        BY <1>1
           DEF StrongInductiveInvariant, Safety,
               OnePendingPersistencePerNode
      <3> QED BY <3>1, <3>2,
                   RemovingRequestsPreservesNodeUniqueness
         DEF OnePendingPersistencePerNode
    <2>3. /\ ProposalSigningRequiresIntent'
          /\ PrepareSigningRequiresIntent'
          /\ CommitSigningRequiresIntent'
          /\ TimeoutSigningRequiresIntent'
          /\ HonestPrepareUniqueness'
          /\ HonestCommitUniqueness'
          /\ HonestTimeoutUniqueness'
          /\ LockBelowHighest'
          /\ DecisionAgreement'
          /\ AppliedRequiresDecision'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, Safety, Crash,
             ProposalSigningRequiresIntent, PrepareSigningRequiresIntent,
             CommitSigningRequiresIntent, TimeoutSigningRequiresIntent,
             HonestPrepareUniqueness, HonestCommitUniqueness,
             HonestTimeoutUniqueness, LockBelowHighest, DecisionAgreement,
             AppliedRequiresDecision
    <2>4. Safety'
      BY <2>1, <2>2, <2>3 DEF Safety
    <2>5. /\ ContextIdentityBindsFrozenEpoch'
          /\ OldContextCertificateRejected'
          /\ ContextParentWasApplied'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, Crash,
             ContextIdentityBindsFrozenEpoch,
             OldContextCertificateRejected, ContextParentWasApplied,
             QcValid, CurrentEpoch
    <2>6. ReducerProvenanceInvariant'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, Crash,
             ReducerProvenanceInvariant, HonestVoteUnique,
             HonestTimeoutUnique, IntentPhasesCorrect,
             PendingVoteWritesAuthorized,
             PendingCertificateWritesAuthorized,
             HonestVoteTransportBacked, QcTransportBacked,
             HonestTimeoutTransportBacked, TcTransportBacked,
             CertificatesBackedByIntents, HonestDurableIntentsSound,
             FormedTimeoutCertificatesSound,
             DurableTimeoutsProtectCommits, HighestAndLockAreCertified,
             VoteIntentFor
    <2> QED BY <2>4, <2>5, <2>6 DEF StrongInductiveInvariant
  <1> QED BY <1>1

THEOREM BeginTimeoutPreservesStrongInvariant ==
  \A node:
    StrongInductiveInvariant /\ BeginTimeout(node)
      => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME NEW node,
              StrongInductiveInvariant,
              BeginTimeout(node)
         PROVE StrongInductiveInvariant'
    <2>1. TypeInvariant'
      BY <1>1, SMT
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             BeginTimeout
    <2>2. OnePendingPersistencePerNode'
      <3>1. RequestsUniqueByNode(AllPendingRequests)
        BY <1>1
           DEF StrongInductiveInvariant, Safety,
               OnePendingPersistencePerNode
      <3>2. node \notin RequestNodeSet(AllPendingRequests)
        BY <1>1, PendingNodesAreAllRequestNodes
           DEF BeginTimeout, NodeIdle
      <3>3. AllPendingRequests'
               = AllPendingRequests \cup {TimeoutRequestFor(node)}
        BY <1>1, Isa DEF BeginTimeout, AllPendingRequests
      <3>4. TimeoutRequestFor(node).node = node
        BY DEF TimeoutRequestFor, TimeoutWal
      <3>5. RequestsUniqueByNode(
               AllPendingRequests \cup {TimeoutRequestFor(node)})
        BY <3>1, <3>2, <3>4,
           NewRequestPreservesNodeUniqueness
      <3> QED BY <3>3, <3>5
         DEF OnePendingPersistencePerNode
    <2>3. Safety'
      BY <1>1, <2>1, <2>2, Isa
         DEF StrongInductiveInvariant, Safety, BeginTimeout,
             ProposalSigningRequiresIntent, PrepareSigningRequiresIntent,
             CommitSigningRequiresIntent, TimeoutSigningRequiresIntent,
             HonestPrepareUniqueness, HonestCommitUniqueness,
             HonestTimeoutUniqueness, LockBelowHighest, DecisionAgreement,
             AppliedRequiresDecision
    <2>4. /\ ContextIdentityBindsFrozenEpoch'
          /\ OldContextCertificateRejected'
          /\ ContextParentWasApplied'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, BeginTimeout,
             ContextIdentityBindsFrozenEpoch,
             OldContextCertificateRejected, ContextParentWasApplied,
             QcValid, CurrentEpoch
    <2>5. PendingVoteWritesAuthorized'
      BY <1>1, SMT
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             BeginTimeout, PendingVoteWritesAuthorized,
             TimeoutRequestFor, LocalTimeoutVoteFor, TimeoutWal,
             CanAppendTimeout, SameTimeoutSlot, SameTimeoutContent
    <2>6. /\ HonestVoteUnique(prepareIntents)'
          /\ HonestVoteUnique(commitIntents)'
          /\ HonestTimeoutUnique(timeoutIntents)'
          /\ IntentPhasesCorrect'
          /\ PendingCertificateWritesAuthorized'
          /\ HonestVoteTransportBacked'
          /\ QcTransportBacked'
          /\ HonestTimeoutTransportBacked'
          /\ TcTransportBacked'
          /\ CertificatesBackedByIntents'
          /\ HonestDurableIntentsSound'
          /\ FormedTimeoutCertificatesSound'
          /\ DurableTimeoutsProtectCommits'
          /\ HighestAndLockAreCertified'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             BeginTimeout, HonestVoteUnique, HonestTimeoutUnique,
             IntentPhasesCorrect, PendingCertificateWritesAuthorized,
             HonestVoteTransportBacked, QcTransportBacked,
             HonestTimeoutTransportBacked, TcTransportBacked,
             CertificatesBackedByIntents, HonestDurableIntentsSound,
             FormedTimeoutCertificatesSound,
             DurableTimeoutsProtectCommits, HighestAndLockAreCertified
    <2>7. ReducerProvenanceInvariant'
      BY <2>5, <2>6 DEF ReducerProvenanceInvariant
    <2> QED BY <2>3, <2>4, <2>7 DEF StrongInductiveInvariant
  <1> QED BY <1>1

THEOREM TimeoutProtectionAppend ==
  \A timeoutVotes, timeoutVote, commits:
    TimeoutIntentProtectsCommits(timeoutVotes, commits)
      /\ TimeoutVoteProtectsCommitSet(timeoutVote, commits)
      => TimeoutIntentProtectsCommits(
           timeoutVotes \cup {timeoutVote}, commits)
BY DEF TimeoutIntentProtectsCommits

THEOREM PersistTimeoutPreservesStrongInvariant ==
  \A request:
    StrongInductiveInvariant /\ PersistTimeout(request)
      => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME NEW request,
              StrongInductiveInvariant,
              PersistTimeout(request)
         PROVE StrongInductiveInvariant'
    <2>1. /\ request.node \in ValidatorIds
          /\ request.vote \in TimeoutVoteRecordSet
          /\ request.node \in Honest
          /\ request.vote.signer = request.node
          /\ CanAppendTimeout(timeoutIntents, request.vote)
          /\ TimeoutVoteProtectsCommitSet(request.vote, commitIntents)
      BY <1>1
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             ReducerProvenanceInvariant, PendingVoteWritesAuthorized,
             PersistTimeout, TimeoutWalSet
    <2>2. TypeInvariant'
      BY <1>1, <2>1, SMT
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             PersistTimeout, TimeoutSign, TimeoutSignSet
    <2>3. OnePendingPersistencePerNode'
      <3>1. RequestsUniqueByNode(AllPendingRequests)
        BY <1>1
           DEF StrongInductiveInvariant, Safety,
               OnePendingPersistencePerNode
      <3>2. AllPendingRequests' \subseteq AllPendingRequests
        BY <1>1, Isa DEF PersistTimeout, AllPendingRequests
      <3> QED BY <3>1, <3>2,
                   RemovingRequestsPreservesNodeUniqueness
         DEF OnePendingPersistencePerNode
    <2>4. HonestTimeoutUnique(timeoutIntents)'
      BY <1>1, <2>1, DurableTimeoutAppendPreservesUniqueness
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             PersistTimeout
    <2>5. DurableTimeoutsProtectCommits'
      BY <1>1, <2>1, TimeoutProtectionAppend
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             DurableTimeoutsProtectCommits, PersistTimeout
    <2>6. TimeoutSigningRequiresIntent'
      BY <1>1, SMT
         DEF StrongInductiveInvariant, Safety, PersistTimeout,
             TimeoutSigningRequiresIntent, TimeoutSign
    <2>7. HonestTimeoutUniqueness'
      BY <2>4
         DEF HonestTimeoutUnique, HonestTimeoutUniqueness,
             SameTimeoutSlot, SameTimeoutContent
    <2>8. /\ ProposalSigningRequiresIntent'
          /\ PrepareSigningRequiresIntent'
          /\ CommitSigningRequiresIntent'
          /\ HonestPrepareUniqueness'
          /\ HonestCommitUniqueness'
          /\ LockBelowHighest'
          /\ DecisionAgreement'
          /\ AppliedRequiresDecision'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, Safety, PersistTimeout,
             ProposalSigningRequiresIntent, PrepareSigningRequiresIntent,
             CommitSigningRequiresIntent, HonestPrepareUniqueness,
             HonestCommitUniqueness, LockBelowHighest, DecisionAgreement,
             AppliedRequiresDecision
    <2>9. Safety'
      BY <2>2, <2>3, <2>6, <2>7, <2>8 DEF Safety
    <2>10. /\ ContextIdentityBindsFrozenEpoch'
           /\ OldContextCertificateRejected'
           /\ ContextParentWasApplied'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, PersistTimeout,
             ContextIdentityBindsFrozenEpoch,
             OldContextCertificateRejected, ContextParentWasApplied,
             QcValid, CurrentEpoch
    <2>11. PendingVoteWritesAuthorized'
      BY <1>1, <2>1, SMT
         DEF StrongInductiveInvariant, Safety, OnePendingPersistencePerNode,
             AllPendingRequests, RequestsUniqueByNode,
             ReducerProvenanceInvariant, PendingVoteWritesAuthorized,
             PersistTimeout, CanAppendTimeout,
             SameTimeoutSlot, SameTimeoutContent
    <2>12. /\ HonestVoteUnique(prepareIntents)'
           /\ HonestVoteUnique(commitIntents)'
           /\ IntentPhasesCorrect'
           /\ PendingCertificateWritesAuthorized'
           /\ HonestVoteTransportBacked'
           /\ QcTransportBacked'
           /\ HonestTimeoutTransportBacked'
           /\ TcTransportBacked'
           /\ CertificatesBackedByIntents'
           /\ HonestDurableIntentsSound'
           /\ FormedTimeoutCertificatesSound'
           /\ HighestAndLockAreCertified'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             PersistTimeout, HonestVoteUnique, IntentPhasesCorrect,
             PendingCertificateWritesAuthorized,
             HonestVoteTransportBacked, QcTransportBacked,
             HonestTimeoutTransportBacked, TcTransportBacked,
             CertificatesBackedByIntents, HonestDurableIntentsSound,
             FormedTimeoutCertificatesSound, HighestAndLockAreCertified
    <2>13. ReducerProvenanceInvariant'
      BY <2>4, <2>5, <2>11, <2>12
         DEF ReducerProvenanceInvariant
    <2> QED BY <2>9, <2>10, <2>13 DEF StrongInductiveInvariant
  <1> QED BY <1>1

=============================================================================
