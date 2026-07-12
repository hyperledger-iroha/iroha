---- MODULE SumeragiV2InductiveProofs ----
EXTENDS SumeragiV2Inductive, SumeragiV2SafetyLemmas,
        SumeragiV2AgreementLemmas, NaturalsInduction

(***************************************************************************
Action-by-action proof that the executable reducer establishes and preserves
its asynchronous provenance.  This module is intentionally separate from the
TLC-loadable invariant vocabulary.
***************************************************************************)

THEOREM NaturalOrderReflexive ==
  \A value \in Nat: value <= value
BY SMT

THEOREM NaturalStrictUpperIsPositive ==
  \A lower, upper \in Nat: lower < upper => upper > 0
BY SMT

THEOREM IntegerOrderChain ==
  \A lower, middle, upper \in Int:
    lower >= 0 /\ middle >= lower /\ upper > middle
      => /\ middle >= 0
         /\ upper >= 1
         /\ lower < upper
BY SMT

THEOREM IntegerWeakStrongOrderChain ==
  \A lower, middle, upper \in Int:
    lower <= middle /\ middle < upper => lower < upper
BY SMT

THEOREM IntegerStrictImpliesWeak ==
  \A lower, upper \in Int: lower < upper => lower <= upper
BY SMT

THEOREM ViewIsNotNoRank ==
  \A roundView \in Views: roundView # NoRank
BY SMT DEF Views, NoRank

THEOREM ViewsAreRanks == Views \subseteq Ranks
BY SMT DEF Views, Ranks, NoRank

THEOREM SubjectsAreSubjectOrNone == Subjects \subseteq SubjectOrNone
BY DEF SubjectOrNone

THEOREM FunctionValueHasCodomain ==
  \A domain, codomain, mapping, key:
    mapping \in [domain -> codomain]
      /\ key \in domain
      => mapping[key] \in codomain
BY Isa

THEOREM FunctionalUpdatePreservesType ==
  \A domain, codomain, mapping, key, value:
    mapping \in [domain -> codomain]
      /\ key \in domain
      /\ value \in codomain
      => [mapping EXCEPT ![key] = value] \in [domain -> codomain]
BY Isa

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
    <2>3. <<>> \in LineagesAt(0)
      BY Isa DEF LineagesAt
    <2>4. ContextRecord(0, <<>>) \in ContextRecords
      BY <2>2, <2>3 DEF ContextRecords
    <2>5. /\ [node \in ValidatorIds |-> 0]
                    \in [ValidatorIds -> Views]
          /\ [node \in ValidatorIds |-> 0]
                    \in [ValidatorIds -> Generations]
          /\ [node \in ValidatorIds |-> NoRank]
                    \in [ValidatorIds -> Ranks]
          /\ [node \in ValidatorIds |-> NoSubject]
                    \in [ValidatorIds -> SubjectOrNone]
      BY <2>2, Isa
    <2>6. /\ context = ContextRecord(0, <<>>)
          /\ context.height = 0
          /\ contextHistory = {context}
          /\ context \in contextHistory
          /\ contextHistory \subseteq ContextRecords
      BY <1>1, <2>4, Isa DEF Init, ContextRecord
    <2>7. /\ proposalIntents \subseteq ProposalRecordSet
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
    <2>8. /\ pendingProposal \subseteq ProposalWalSet
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
    <2>9. /\ height \in Heights
          /\ nodeView \in [ValidatorIds -> Views]
          /\ generation \in [ValidatorIds -> Generations]
          /\ up \subseteq ValidatorIds
          /\ gst \in BOOLEAN
          /\ lockRank \in [ValidatorIds -> Ranks]
          /\ lockSubject \in [ValidatorIds -> SubjectOrNone]
          /\ highestRank \in [ValidatorIds -> Ranks]
          /\ highestSubject \in [ValidatorIds -> SubjectOrNone]
      BY <1>1, <2>2, <2>5, Isa DEF Init
    <2> QED BY <1>1, <2>1, <2>6, <2>7, <2>8, <2>9
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
          /\ CertificatePhasesCorrect
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
             IntentPhasesCorrect, CertificatePhasesCorrect,
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

THEOREM InitEstablishesLineageInvariant == Init => LineageInvariant
BY Isa
   DEF Init, LineageInvariant, PrepareLineageSound,
       LocksCoverOwnCommits, CurrentIntentViewsBound,
       HonestCommitIntentPrepared, CommitIntentsPreparedBy,
       CertificatePhasesCorrect, DurableIntentsDoNotAnticipateHeight,
       ContextRecord

THEOREM InitEstablishesStrongInductiveInvariant ==
  Init => StrongInductiveInvariant
BY InitEstablishesReleaseSafety,
   InitEstablishesReducerProvenance,
   InitEstablishesContextSafety,
   InitEstablishesLineageInvariant
   DEF StrongInductiveInvariant

THEOREM UnchangedLineageVarsPreservesLineageInvariant ==
  LineageInvariant /\ UNCHANGED LineageVars => LineageInvariant'
BY Isa
   DEF LineageInvariant, LineageVars, PrepareLineageSound,
       PrepareCarriesHigherSafeQc, LocksCoverOwnCommits,
       CurrentIntentViewsBound, HonestCommitIntentPrepared,
       CertificatePhasesCorrect, DurableIntentsDoNotAnticipateHeight

THEOREM UnchangedProvenanceVarsPreservesReducerProvenance ==
  ReducerProvenanceInvariant /\ UNCHANGED ProvenanceVars
    => ReducerProvenanceInvariant'
PROOF
  <1>1. ASSUME ReducerProvenanceInvariant,
              UNCHANGED ProvenanceVars
         PROVE ReducerProvenanceInvariant'
    <2>1. /\ HonestVoteUnique(prepareIntents)'
          /\ HonestVoteUnique(commitIntents)'
          /\ HonestTimeoutUnique(timeoutIntents)'
          /\ IntentPhasesCorrect'
      BY <1>1, Isa
         DEF ProvenanceVars, ReducerProvenanceInvariant,
             HonestVoteUnique, HonestTimeoutUnique, IntentPhasesCorrect,
             SameVoteSlot, SameTimeoutSlot, SameTimeoutContent
    <2>2. PendingVoteWritesAuthorized'
      BY <1>1, Isa
         DEF ProvenanceVars, ReducerProvenanceInvariant,
             PendingVoteWritesAuthorized, PrepareCarriesHigherSafeQc,
             NodeTimedOut
    <2>3. PendingCertificateWritesAuthorized'
      BY <1>1, Isa
         DEF ProvenanceVars, ReducerProvenanceInvariant,
             PendingCertificateWritesAuthorized, TCValid, HighRefValid,
             CurrentEpoch, CurrentVoters
    <2>4. /\ HonestVoteTransportBacked'
          /\ QcTransportBacked'
          /\ HonestTimeoutTransportBacked'
          /\ TcTransportBacked'
      <3>1. HonestVoteTransportBacked'
        BY <1>1, Isa
           DEF ProvenanceVars, ReducerProvenanceInvariant,
               HonestVoteTransportBacked, VoteIntentFor
      <3>2. QcTransportBacked'
        BY <1>1, Isa
           DEF ProvenanceVars, ReducerProvenanceInvariant,
               QcTransportBacked
      <3>3. HonestTimeoutTransportBacked'
        BY <1>1, Isa
           DEF ProvenanceVars, ReducerProvenanceInvariant,
               HonestTimeoutTransportBacked
      <3>4. TcTransportBacked'
        BY <1>1, Isa
           DEF ProvenanceVars, ReducerProvenanceInvariant,
               TcTransportBacked, TCValid, HighRefValid,
               CurrentEpoch, CurrentVoters
      <3> QED BY <3>1, <3>2, <3>3, <3>4
    <2>5. /\ CertificatesBackedByIntents'
          /\ HonestDurableIntentsSound'
          /\ FormedTimeoutCertificatesSound'
          /\ DurableTimeoutsProtectCommits'
          /\ HighestAndLockAreCertified'
      <3>1. CertificatesBackedByIntents'
        BY <1>1, Isa
           DEF ProvenanceVars, ReducerProvenanceInvariant,
               CertificatesBackedByIntents
      <3>2. HonestDurableIntentsSound'
        BY <1>1, Isa
           DEF ProvenanceVars, ReducerProvenanceInvariant,
               HonestDurableIntentsSound
      <3>3. FormedTimeoutCertificatesSound'
        BY <1>1, Isa
           DEF ProvenanceVars, ReducerProvenanceInvariant,
               FormedTimeoutCertificatesSound
      <3>4. DurableTimeoutsProtectCommits'
        BY <1>1, Isa
           DEF ProvenanceVars, ReducerProvenanceInvariant,
               DurableTimeoutsProtectCommits
      <3>5. HighestAndLockAreCertified'
        BY <1>1, Isa
           DEF ProvenanceVars, ReducerProvenanceInvariant,
               HighestAndLockAreCertified
      <3> QED BY <3>1, <3>2, <3>3, <3>4, <3>5
    <2> QED BY <2>1, <2>2, <2>3, <2>4, <2>5
       DEF ReducerProvenanceInvariant
  <1> QED BY <1>1

THEOREM UnchangedVoteIndependentProvenancePreserves ==
  ReducerProvenanceWithoutVoteTransport
    /\ UNCHANGED ProvenanceWithoutVoteTransportVars
    => ReducerProvenanceWithoutVoteTransport'
PROOF
  <1>1. ASSUME ReducerProvenanceWithoutVoteTransport,
              UNCHANGED ProvenanceWithoutVoteTransportVars
         PROVE ReducerProvenanceWithoutVoteTransport'
    <2>1. /\ HonestVoteUnique(prepareIntents)'
          /\ HonestVoteUnique(commitIntents)'
          /\ HonestTimeoutUnique(timeoutIntents)'
          /\ IntentPhasesCorrect'
          /\ PendingVoteWritesAuthorized'
          /\ PendingCertificateWritesAuthorized'
      BY <1>1, Isa
         DEF ReducerProvenanceWithoutVoteTransport,
             ProvenanceWithoutVoteTransportVars, HonestVoteUnique,
             HonestTimeoutUnique, IntentPhasesCorrect,
             PendingVoteWritesAuthorized,
             PendingCertificateWritesAuthorized,
             PrepareCarriesHigherSafeQc, NodeTimedOut,
             TCValid, HighRefValid, CurrentEpoch, CurrentVoters
    <2>2. /\ QcTransportBacked'
          /\ HonestTimeoutTransportBacked'
          /\ TcTransportBacked'
      BY <1>1, Isa
         DEF ReducerProvenanceWithoutVoteTransport,
             ProvenanceWithoutVoteTransportVars, QcTransportBacked,
             HonestTimeoutTransportBacked, TcTransportBacked,
             TCValid, HighRefValid, CurrentEpoch, CurrentVoters
    <2>3. /\ CertificatesBackedByIntents'
          /\ HonestDurableIntentsSound'
          /\ FormedTimeoutCertificatesSound'
          /\ DurableTimeoutsProtectCommits'
          /\ HighestAndLockAreCertified'
      <3>1. CertificatesBackedByIntents'
        BY <1>1, Isa
           DEF ReducerProvenanceWithoutVoteTransport,
               ProvenanceWithoutVoteTransportVars,
               CertificatesBackedByIntents
      <3>2. HonestDurableIntentsSound'
        BY <1>1, Isa
           DEF ReducerProvenanceWithoutVoteTransport,
               ProvenanceWithoutVoteTransportVars,
               HonestDurableIntentsSound
      <3>3. FormedTimeoutCertificatesSound'
        BY <1>1, Isa
           DEF ReducerProvenanceWithoutVoteTransport,
               ProvenanceWithoutVoteTransportVars,
               FormedTimeoutCertificatesSound
      <3>4. DurableTimeoutsProtectCommits'
        BY <1>1, Isa
           DEF ReducerProvenanceWithoutVoteTransport,
               ProvenanceWithoutVoteTransportVars,
               DurableTimeoutsProtectCommits
      <3>5. HighestAndLockAreCertified'
        BY <1>1, Isa
           DEF ReducerProvenanceWithoutVoteTransport,
               ProvenanceWithoutVoteTransportVars,
               HighestAndLockAreCertified
      <3> QED BY <3>1, <3>2, <3>3, <3>4, <3>5
    <2> QED BY <2>1, <2>2, <2>3
       DEF ReducerProvenanceWithoutVoteTransport
  <1> QED BY <1>1

THEOREM PersistPreparePreservesLineageInvariant ==
  \A request:
    TypeInvariant /\ LineageInvariant /\ PendingVoteWritesAuthorized
      /\ PersistPrepare(request)
      => LineageInvariant'
PROOF
  <1>1. ASSUME NEW request,
              TypeInvariant,
              LineageInvariant,
              PendingVoteWritesAuthorized,
              PersistPrepare(request)
         PROVE LineageInvariant'
    <2>1. request \in pendingPrepare
      BY <1>1 DEF PersistPrepare
    <2>2. /\ request.vote.signer \in Honest
          /\ PrepareCarriesHigherSafeQc(request.vote)
          /\ request.vote.context = context
          /\ request.vote.view = nodeView[request.vote.signer]
      BY <1>1, <2>1
         DEF PendingVoteWritesAuthorized
    <2>3. /\ prepareIntents' = prepareIntents \cup {request.vote}
          /\ context' = context
          /\ nodeView' = nodeView
          /\ commitIntents' = commitIntents
          /\ timeoutIntents' = timeoutIntents
          /\ prepareQCs' = prepareQCs
          /\ commitQCs' = commitQCs
          /\ lockRank' = lockRank
          /\ lockSubject' = lockSubject
      BY <1>1 DEF PersistPrepare
    <2>4. PrepareLineageSound'
      <3>1. ASSUME NEW vote \in prepareIntents',
                    vote.signer \in Honest
             PROVE PrepareCarriesHigherSafeQc(vote)'
        <4>1. vote \in prepareIntents \/ vote = request.vote
          BY <2>3, <3>1
        <4>2. CASE vote \in prepareIntents
          BY <1>1, <2>3, <3>1, <4>2
             DEF LineageInvariant, PrepareLineageSound,
                 PrepareCarriesHigherSafeQc
        <4>3. CASE vote = request.vote
          BY <2>2, <2>3, <3>1, <4>3
             DEF PrepareCarriesHigherSafeQc
        <4> QED BY <4>1, <4>2, <4>3
      <3> QED BY <3>1 DEF PrepareLineageSound
    <2>5. LocksCoverOwnCommits'
      BY <1>1, <2>3, IsaM("blast")
         DEF LineageInvariant, LocksCoverOwnCommits
    <2>6. \A vote \in prepareIntents':
              (vote.signer \in Honest /\ vote.context = context')
                => vote.view <= nodeView'[vote.signer]
      <3>1. ASSUME NEW vote \in prepareIntents',
                    vote.signer \in Honest,
                    vote.context = context'
             PROVE vote.view <= nodeView'[vote.signer]
        <4>1. vote \in prepareIntents \/ vote = request.vote
          BY <2>3, <3>1
        <4>2. CASE vote \in prepareIntents
          BY <1>1, <2>3, <3>1, <4>2
             DEF LineageInvariant, CurrentIntentViewsBound
        <4>3. CASE vote = request.vote
          <5>1. /\ vote.view = request.vote.view
                /\ vote.signer = request.vote.signer
            BY <4>3
          <5>2. request.vote.view =
                   nodeView[request.vote.signer]
            BY <2>2
          <5>3. nodeView'[vote.signer] =
                   nodeView[request.vote.signer]
            BY <2>3, <5>1
          <5>4. vote.view = nodeView'[vote.signer]
            BY <5>1, <5>2, <5>3
          <5>5. vote.signer \in ValidatorIds
            BY <1>1, <3>1, SMT
               DEF TypeInvariant, ModelConfiguration,
                   QuorumConfiguration
          <5>6. nodeView[vote.signer] \in Views
            BY <1>1, <5>5 DEF TypeInvariant
          <5>7. nodeView'[vote.signer] \in Nat
            BY <2>3, <5>3, <5>6, SMT DEF Views
          <5> QED BY <5>4, <5>7, NaturalOrderReflexive
        <4> QED BY <4>1, <4>2, <4>3
      <3> QED BY <3>1
    <2>7. \A vote \in commitIntents':
              (vote.signer \in Honest /\ vote.context = context')
                => vote.view <= nodeView'[vote.signer]
      <3>1. ASSUME NEW vote \in commitIntents',
                    vote.signer \in Honest,
                    vote.context = context'
             PROVE vote.view <= nodeView'[vote.signer]
        <4>1. vote \in commitIntents
          BY <2>3, <3>1
        <4>2. vote.context = context
          BY <2>3, <3>1
        <4>3. vote.view <= nodeView[vote.signer]
          BY <1>1, <3>1, <4>1, <4>2
             DEF LineageInvariant, HonestCommitIntentPrepared
        <4>4. nodeView'[vote.signer] = nodeView[vote.signer]
          BY <2>3
        <4> QED BY <4>3, <4>4
      <3> QED BY <3>1
    <2>8. CurrentIntentViewsBound'
      BY <1>1, <2>3, <2>6, Isa
         DEF LineageInvariant, CurrentIntentViewsBound
    <2>9. HonestCommitIntentPrepared'
      BY <1>1, <2>3, Isa
         DEF LineageInvariant, HonestCommitIntentPrepared
    <2>10. CertificatePhasesCorrect'
      BY <1>1, <2>3, Isa
         DEF LineageInvariant, CertificatePhasesCorrect
    <2>11. DurableIntentsDoNotAnticipateHeight'
      <3>1. DurableIntentsDoNotAnticipateHeight
        BY <1>1 DEF LineageInvariant
      <3>2. request.vote.context.height <= height
        BY <1>1, <2>2, SMT
           DEF TypeInvariant, Heights
      <3> QED BY <1>1, <2>3, <3>1, <3>2, Isa
         DEF DurableIntentsDoNotAnticipateHeight, PersistPrepare
    <2> QED BY <2>4, <2>5, <2>8, <2>9, <2>10, <2>11
       DEF LineageInvariant
  <1> QED BY <1>1

THEOREM CommitPreparationIsMonotone ==
  \A commits, before, after:
    CommitIntentsPreparedBy(commits, before)
      /\ before \subseteq after
      => CommitIntentsPreparedBy(commits, after)
BY DEF CommitIntentsPreparedBy

THEOREM FormPrepareQCPreservesLineageInvariant ==
  \A node, roundView, subject:
    LineageInvariant /\ FormPrepareQC(node, roundView, subject)
      => LineageInvariant'
PROOF
  <1>1. ASSUME NEW node, NEW roundView, NEW subject,
              LineageInvariant,
              FormPrepareQC(node, roundView, subject)
         PROVE LineageInvariant'
    <2>1. /\ prepareQCs \subseteq prepareQCs'
          /\ prepareIntents' = prepareIntents
          /\ commitIntents' = commitIntents
          /\ timeoutIntents' = timeoutIntents
          /\ commitQCs' = commitQCs
          /\ context' = context
          /\ nodeView' = nodeView
          /\ lockRank' = lockRank
          /\ lockSubject' = lockSubject
          /\ \A qc \in prepareQCs' \ prepareQCs:
               qc.phase = "Prepare"
      BY <1>1, SMT DEF FormPrepareQC, QC
    <2>2. PrepareLineageSound'
      <3>1. ASSUME NEW vote \in prepareIntents',
                    vote.signer \in Honest,
                    NEW commitVote \in commitIntents',
                    /\ vote.signer \in Honest
                    /\ commitVote.signer = vote.signer
                    /\ commitVote.context = vote.context
                    /\ commitVote.phase = "Commit"
                    /\ commitVote.view < vote.view
                    /\ commitVote.subject # vote.subject
             PROVE \E qc \in prepareQCs':
                     /\ qc.context = vote.context
                     /\ qc.phase = "Prepare"
                     /\ commitVote.view < qc.view
                     /\ qc.view < vote.view
                     /\ qc.subject = vote.subject
        <4>1. PrepareCarriesHigherSafeQc(vote)
          BY <1>1, <2>1, <3>1
             DEF LineageInvariant, PrepareLineageSound
        <4>2. PICK qc \in prepareQCs:
                 /\ qc.context = vote.context
                 /\ qc.phase = "Prepare"
                 /\ commitVote.view < qc.view
                 /\ qc.view < vote.view
                 /\ qc.subject = vote.subject
          BY <2>1, <3>1, <4>1 DEF PrepareCarriesHigherSafeQc
        <4> QED BY <2>1, <4>2
      <3> QED BY <3>1
         DEF PrepareLineageSound, PrepareCarriesHigherSafeQc
    <2>3. /\ LocksCoverOwnCommits'
          /\ CurrentIntentViewsBound'
      BY <1>1, <2>1, Isa
         DEF LineageInvariant, LocksCoverOwnCommits,
             CurrentIntentViewsBound
    <2>4. HonestCommitIntentPrepared'
      BY <1>1, <2>1, CommitPreparationIsMonotone
         DEF LineageInvariant, HonestCommitIntentPrepared
    <2>5. CertificatePhasesCorrect'
      BY <1>1, <2>1, SMT
         DEF LineageInvariant, CertificatePhasesCorrect
    <2>6. DurableIntentsDoNotAnticipateHeight'
      BY <1>1, <2>1, Isa
         DEF LineageInvariant, DurableIntentsDoNotAnticipateHeight,
             FormPrepareQC
    <2> QED BY <2>2, <2>3, <2>4, <2>5, <2>6
       DEF LineageInvariant
  <1> QED BY <1>1

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
      BY <1>1, UnchangedProvenanceVarsPreservesReducerProvenance
         DEF StrongInductiveInvariant, SetGST, ProvenanceVars
    <2>8. LineageInvariant'
      BY <1>1, UnchangedLineageVarsPreservesLineageInvariant
         DEF StrongInductiveInvariant, SetGST, LineageVars
    <2> QED BY <2>3, <2>4, <2>5, <2>6, <2>7, <2>8
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
      BY <1>1, SMT
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
      BY <1>1, IsaM("blast")
         DEF StrongInductiveInvariant, ProofRelevantVars,
             ContextIdentityBindsFrozenEpoch,
             OldContextCertificateRejected, ContextParentWasApplied,
             QcValid, CurrentEpoch
    <2>3. ReducerProvenanceInvariant'
      BY <1>1, UnchangedProvenanceVarsPreservesReducerProvenance
         DEF StrongInductiveInvariant, ProofRelevantVars, ProvenanceVars
    <2>4. LineageInvariant'
      BY <1>1, UnchangedLineageVarsPreservesLineageInvariant
         DEF StrongInductiveInvariant, ProofRelevantVars, LineageVars
    <2> QED BY <2>1, <2>2, <2>3, <2>4
       DEF StrongInductiveInvariant
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

THEOREM BodyHeldIsMonotone ==
  \A before, after, node, bodyContext, subject:
    before \subseteq after
      /\ BodyHeldBy(before, node, bodyContext, subject)
      => BodyHeldBy(after, node, bodyContext, subject)
BY DEF BodyHeldBy

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
      BY <1>1, SMT
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
      BY <1>1, SMT
         DEF StrongInductiveInvariant, ProofRelevantWithoutDurableVars,
             ContextIdentityBindsFrozenEpoch,
             OldContextCertificateRejected, ContextParentWasApplied,
             QcValid, CurrentEpoch
    <2>3. HonestDurableIntentsSound'
      BY <1>1, HonestIntentSoundIsMonotoneInDurableBodies
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             HonestDurableIntentsSound,
             ProofRelevantWithoutDurableVars
    <2>4. PendingVoteWritesAuthorized'
      <3>1. PendingVoteWritesAuthorized
        BY <1>1 DEF StrongInductiveInvariant, ReducerProvenanceInvariant
      <3>2. \A request \in pendingPrepare':
               /\ request.node \in Honest
               /\ request.vote.phase = "Prepare"
               /\ request.vote.signer = request.node
               /\ request.vote.context = context'
               /\ request.vote.view = nodeView'[request.node]
               /\ request.vote.subject \in ValidSubjects
               /\ BodyHeldBy(durableBodies', request.node,
                             request.vote.context, request.vote.subject)
               /\ CanAppendVote(prepareIntents', request.vote)
               /\ PrepareCarriesHigherSafeQc(request.vote)'
        <4>1. ASSUME NEW request \in pendingPrepare'
               PROVE /\ request.node \in Honest
                     /\ request.vote.phase = "Prepare"
                     /\ request.vote.signer = request.node
                     /\ request.vote.context = context'
                     /\ request.vote.view = nodeView'[request.node]
                     /\ request.vote.subject \in ValidSubjects
                     /\ BodyHeldBy(durableBodies', request.node,
                                   request.vote.context,
                                   request.vote.subject)
                     /\ CanAppendVote(prepareIntents', request.vote)
                     /\ PrepareCarriesHigherSafeQc(request.vote)'
          <5>1. BodyHeldBy(durableBodies, request.node,
                          request.vote.context, request.vote.subject)
            BY <1>1, <3>1, <4>1
               DEF ProofRelevantWithoutDurableVars,
                   PendingVoteWritesAuthorized
          <5>2. BodyHeldBy(durableBodies', request.node,
                          request.vote.context, request.vote.subject)
            BY <1>1, <5>1, BodyHeldIsMonotone
          <5> QED BY <1>1, <3>1, <4>1, <5>2, Isa
             DEF ProofRelevantWithoutDurableVars,
                 PendingVoteWritesAuthorized,
                 PrepareCarriesHigherSafeQc
        <4> QED BY <4>1
      <3>3. \A request \in pendingLockCommit':
               /\ request.node \in Honest
               /\ request.vote.phase = "Commit"
               /\ request.vote.signer = request.node
               /\ request.vote.context = context'
               /\ request.vote.context = request.qc.context
               /\ request.vote.view = request.qc.view
               /\ request.vote.subject = request.qc.subject
               /\ request.qc.phase = "Prepare"
               /\ request.qc \in prepareQCs'
               /\ request.vote.view = nodeView'[request.node]
               /\ ~NodeTimedOut(request.node, request.vote.view)'
               /\ request.vote.subject \in ValidSubjects
               /\ BodyHeldBy(durableBodies', request.node,
                             request.vote.context, request.vote.subject)
               /\ request.qc.view >= lockRank'[request.node]
               /\ (request.qc.view = lockRank'[request.node]
                     => request.qc.subject = lockSubject'[request.node])
               /\ CanAppendVote(commitIntents', request.vote)
        <4>1. ASSUME NEW request \in pendingLockCommit'
               PROVE /\ request.node \in Honest
                     /\ request.vote.phase = "Commit"
                     /\ request.vote.signer = request.node
                     /\ request.vote.context = context'
                     /\ request.vote.context = request.qc.context
                     /\ request.vote.view = request.qc.view
                     /\ request.vote.subject = request.qc.subject
                     /\ request.qc.phase = "Prepare"
                     /\ request.qc \in prepareQCs'
                     /\ request.vote.view = nodeView'[request.node]
                     /\ ~NodeTimedOut(request.node, request.vote.view)'
                     /\ request.vote.subject \in ValidSubjects
                     /\ BodyHeldBy(durableBodies', request.node,
                                   request.vote.context,
                                   request.vote.subject)
                     /\ request.qc.view >= lockRank'[request.node]
                     /\ (request.qc.view = lockRank'[request.node]
                           => request.qc.subject = lockSubject'[request.node])
                     /\ CanAppendVote(commitIntents', request.vote)
          <5>1. BodyHeldBy(durableBodies, request.node,
                          request.vote.context, request.vote.subject)
            BY <1>1, <3>1, <4>1
               DEF ProofRelevantWithoutDurableVars,
                   PendingVoteWritesAuthorized
          <5>2. BodyHeldBy(durableBodies', request.node,
                          request.vote.context, request.vote.subject)
            BY <1>1, <5>1, BodyHeldIsMonotone
          <5>3. request \in pendingLockCommit
            BY <1>1, <4>1 DEF ProofRelevantWithoutDurableVars
          <5>4. /\ request.node \in Honest
                 /\ request.vote.phase = "Commit"
                 /\ request.vote.signer = request.node
                 /\ request.vote.context = context
                 /\ request.vote.context = request.qc.context
                 /\ request.vote.view = request.qc.view
                 /\ request.vote.subject = request.qc.subject
                 /\ request.qc.phase = "Prepare"
                 /\ request.qc \in prepareQCs
                 /\ request.vote.view = nodeView[request.node]
                 /\ ~NodeTimedOut(request.node, request.vote.view)
                 /\ request.vote.subject \in ValidSubjects
                 /\ request.qc.view >= lockRank[request.node]
                 /\ (request.qc.view = lockRank[request.node]
                       => request.qc.subject = lockSubject[request.node])
                 /\ CanAppendVote(commitIntents, request.vote)
            BY <3>1, <5>3 DEF PendingVoteWritesAuthorized
          <5>5. /\ context' = context
                 /\ nodeView' = nodeView
                 /\ timeoutIntents' = timeoutIntents
                 /\ prepareQCs' = prepareQCs
                 /\ lockRank' = lockRank
                 /\ lockSubject' = lockSubject
                 /\ commitIntents' = commitIntents
            BY <1>1 DEF ProofRelevantWithoutDurableVars
          <5>6. /\ request.node \in Honest
                 /\ request.vote.phase = "Commit"
                 /\ request.vote.signer = request.node
                 /\ request.vote.context = context'
                 /\ request.vote.context = request.qc.context
                 /\ request.vote.view = request.qc.view
                 /\ request.vote.subject = request.qc.subject
                 /\ request.qc.phase = "Prepare"
                 /\ request.qc \in prepareQCs'
                 /\ request.vote.view = nodeView'[request.node]
                 /\ ~NodeTimedOut(request.node, request.vote.view)'
                 /\ request.vote.subject \in ValidSubjects
                 /\ request.qc.view >= lockRank'[request.node]
                 /\ (request.qc.view = lockRank'[request.node]
                       => request.qc.subject = lockSubject'[request.node])
                 /\ CanAppendVote(commitIntents', request.vote)
            BY <5>4, <5>5, Isa DEF NodeTimedOut
          <5> QED BY <5>2, <5>6
        <4> QED BY <4>1
      <3>4. \A request \in pendingTimeout':
               /\ request.node \in Honest
               /\ request.vote.signer = request.node
               /\ request.vote.context = context'
               /\ request.vote.view = nodeView'[request.node]
               /\ CanAppendTimeout(timeoutIntents', request.vote)
               /\ TimeoutVoteProtectsCommitSet(request.vote,
                                               commitIntents')
        BY <1>1, <3>1, Isa
           DEF ProofRelevantWithoutDurableVars,
               PendingVoteWritesAuthorized, NodeTimedOut
      <3> QED BY <3>2, <3>3, <3>4
         DEF PendingVoteWritesAuthorized, NodeTimedOut
    <2>5. /\ HonestVoteUnique(prepareIntents)'
          /\ HonestVoteUnique(commitIntents)'
          /\ HonestTimeoutUnique(timeoutIntents)'
          /\ IntentPhasesCorrect'
          /\ PendingCertificateWritesAuthorized'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             ProofRelevantWithoutDurableVars, HonestVoteUnique,
             HonestTimeoutUnique, IntentPhasesCorrect,
             PendingCertificateWritesAuthorized, TCValid, HighRefValid,
             CurrentEpoch, CurrentVoters
    <2>6. /\ HonestVoteTransportBacked'
          /\ QcTransportBacked'
          /\ HonestTimeoutTransportBacked'
          /\ TcTransportBacked'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             ProofRelevantWithoutDurableVars,
             HonestVoteTransportBacked, QcTransportBacked,
             HonestTimeoutTransportBacked, TcTransportBacked,
             VoteIntentFor, TCValid, HighRefValid,
             CurrentEpoch, CurrentVoters
    <2>7. CertificatesBackedByIntents'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             ProofRelevantWithoutDurableVars,
             CertificatesBackedByIntents
    <2>8. FormedTimeoutCertificatesSound'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             ProofRelevantWithoutDurableVars,
             FormedTimeoutCertificatesSound
    <2>9. /\ DurableTimeoutsProtectCommits'
          /\ HighestAndLockAreCertified'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             ProofRelevantWithoutDurableVars,
             DurableTimeoutsProtectCommits, HighestAndLockAreCertified
    <2>10. ReducerProvenanceInvariant'
      BY <2>3, <2>4, <2>5, <2>6, <2>7, <2>8, <2>9
         DEF ReducerProvenanceInvariant
    <2>11. LineageInvariant'
      BY <1>1, UnchangedLineageVarsPreservesLineageInvariant
         DEF StrongInductiveInvariant, ProofRelevantWithoutDurableVars,
             LineageVars
    <2> QED BY <2>1, <2>2, <2>10, <2>11
       DEF StrongInductiveInvariant
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

THEOREM DistinctUniqueRequestsHaveDistinctNodes ==
  \A requests, left, right:
    RequestsUniqueByNode(requests)
      /\ left \in requests
      /\ right \in requests
      /\ left # right
      => left.node # right.node
BY SMT DEF RequestsUniqueByNode

THEOREM SafeProposalCarriesCommitLineage ==
  \A node, proposal:
    node \in ValidatorIds
      /\ TypeInvariant
      /\ LineageInvariant
      /\ HighestAndLockAreCertified
      /\ ProposalValidFor(node, proposal)
      /\ lockRank[node] < proposal.view
      => PrepareCarriesHigherSafeQc(PrepareVoteFor(node, proposal))
PROOF
  <1>1. ASSUME NEW node, NEW proposal,
              node \in ValidatorIds,
              TypeInvariant,
              LineageInvariant,
              HighestAndLockAreCertified,
              ProposalValidFor(node, proposal),
              lockRank[node] < proposal.view
         PROVE PrepareCarriesHigherSafeQc(
                 PrepareVoteFor(node, proposal))
    <2>1. ASSUME NEW commitVote \in commitIntents,
                  /\ PrepareVoteFor(node, proposal).signer \in Honest
                  /\ commitVote.signer =
                       PrepareVoteFor(node, proposal).signer
                  /\ commitVote.context =
                       PrepareVoteFor(node, proposal).context
                  /\ commitVote.phase = "Commit"
                  /\ commitVote.view <
                       PrepareVoteFor(node, proposal).view
                  /\ commitVote.subject #
                       PrepareVoteFor(node, proposal).subject
           PROVE \E qc \in prepareQCs:
                   /\ qc.context = PrepareVoteFor(node, proposal).context
                   /\ qc.phase = "Prepare"
                   /\ commitVote.view < qc.view
                   /\ qc.view < PrepareVoteFor(node, proposal).view
                   /\ qc.subject =
                        PrepareVoteFor(node, proposal).subject
      <3>1. /\ commitVote.signer = node
            /\ commitVote.context = context
            /\ commitVote.subject # proposal.subject
            /\ PrepareVoteFor(node, proposal).context = context
            /\ PrepareVoteFor(node, proposal).view = proposal.view
            /\ PrepareVoteFor(node, proposal).subject = proposal.subject
        BY <1>1, <2>1
           DEF PrepareVoteFor, Vote, ProposalValidFor
      <3>2. /\ lockRank[node] >= commitVote.view
            /\ (lockRank[node] = commitVote.view
                  => lockSubject[node] = commitVote.subject)
        BY <1>1, <2>1, <3>1 DEF LineageInvariant, LocksCoverOwnCommits
      <3>3. /\ commitVote.view \in Views
            /\ commitVote.view \in Nat
        BY <1>1, <2>1, SMT
           DEF TypeInvariant, VoteRecordSet, Views,
               ModelConfiguration
      <3>4. lockRank[node] # NoRank
        BY <3>2, <3>3, SMT DEF Views, NoRank
      <3>5. CASE lockSubject[node] = proposal.subject
        <4>1. lockRank[node] > commitVote.view
          BY <3>1, <3>2, <3>5, SMT
        <4>2. \E qc \in prepareQCs:
                 /\ qc.context = context
                 /\ qc.view = lockRank[node]
                 /\ qc.subject = lockSubject[node]
          BY <1>1, <3>4 DEF HighestAndLockAreCertified
        <4>3. PICK qc \in prepareQCs:
                 /\ qc.context = context
                 /\ qc.view = lockRank[node]
                 /\ qc.subject = lockSubject[node]
          BY <4>2
        <4>4. qc.phase = "Prepare"
          BY <1>1, <4>3
             DEF LineageInvariant, CertificatePhasesCorrect
        <4>5. /\ qc.context = PrepareVoteFor(node, proposal).context
              /\ qc.phase = "Prepare"
              /\ commitVote.view < qc.view
              /\ qc.view < PrepareVoteFor(node, proposal).view
              /\ qc.subject = PrepareVoteFor(node, proposal).subject
          BY <1>1, <3>1, <3>5, <4>1, <4>3, <4>4
        <4> QED BY <4>3, <4>5
      <3>6. CASE lockSubject[node] # proposal.subject
        <4>1. /\ proposal.justifyRank > lockRank[node]
              /\ proposal.justifySubject = proposal.subject
          BY <1>1, <3>4, <3>6
             DEF ProposalValidFor, SafeToPrepare
        <4>2. proposal.view > 0
          <5>1. commitVote.view < proposal.view
            BY <2>1, <3>1
          <5>2. /\ proposal.view \in Views
                /\ proposal.view \in Nat
            BY <1>1, <3>1, SMT
               DEF TypeInvariant, ProposalValidFor, Views
          <5>3. proposal.view > 0
            BY <3>3, <5>1, <5>2, NaturalStrictUpperIsPositive
          <5> QED BY <5>3
        <4>3. /\ proposal.justifyRank < proposal.view
              /\ \E qc \in prepareQCs:
                   /\ qc.context = context
                   /\ qc.view = proposal.justifyRank
                   /\ qc.subject = proposal.justifySubject
          <5>1. ProposalJustified(node, proposal)
            BY <1>1 DEF ProposalValidFor
          <5>2. proposal.justifyRank # NoRank
            <6>1. /\ commitVote.view >= 0
                  /\ lockRank[node] >= commitVote.view
                  /\ proposal.justifyRank > lockRank[node]
              BY <3>2, <3>3, <4>1, SMT
            <6>2. /\ commitVote.view \in Int
                  /\ lockRank[node] \in Int
                  /\ proposal.justifyRank \in Int
              BY <1>1, <3>3, <4>2, <5>1, SMT
                 DEF TypeInvariant, ModelConfiguration, Ranks, Views,
                     ProposalJustified, HighRefValid, NoRank
            <6>3. /\ lockRank[node] >= 0
                  /\ proposal.justifyRank >= 1
                  /\ commitVote.view < proposal.justifyRank
              BY <6>1, <6>2, IntegerOrderChain
            <6> QED BY <6>3, SMT DEF NoRank
          <5>3. /\ proposal.justifyRank < proposal.view
                /\ HighRefValid(proposal.justifyRank,
                                proposal.justifySubject)
            BY <4>2, <5>1 DEF ProposalJustified
          <5> QED BY <5>2, <5>3 DEF HighRefValid
        <4>4. PICK qc \in prepareQCs:
                 /\ qc.context = context
                 /\ qc.view = proposal.justifyRank
                 /\ qc.subject = proposal.justifySubject
          BY <4>3
        <4>5. qc.phase = "Prepare"
          BY <1>1, <4>4
             DEF LineageInvariant, CertificatePhasesCorrect
        <4>6. /\ qc.context = PrepareVoteFor(node, proposal).context
              /\ qc.phase = "Prepare"
              /\ commitVote.view < qc.view
              /\ qc.view < PrepareVoteFor(node, proposal).view
              /\ qc.subject = PrepareVoteFor(node, proposal).subject
          <5>1. qc.context = PrepareVoteFor(node, proposal).context
            BY <3>1, <4>4
          <5>2. qc.phase = "Prepare"
            BY <4>5
          <5>3. /\ commitVote.view \in Int
                /\ lockRank[node] \in Int
                /\ qc.view \in Int
            BY <1>1, <3>3, <4>4, SMT
               DEF TypeInvariant, ModelConfiguration, Ranks, Views,
                   QcRecordSet, NoRank
          <5>4. commitVote.view < qc.view
            BY <3>2, <4>1, <4>4, <5>3, IntegerOrderChain
          <5>5. qc.view < PrepareVoteFor(node, proposal).view
            BY <3>1, <4>3, <4>4
          <5>6. qc.subject = PrepareVoteFor(node, proposal).subject
            BY <3>1, <4>1, <4>4
          <5> QED BY <5>1, <5>2, <5>4, <5>5, <5>6
        <4> QED BY <4>4, <4>6
      <3> QED BY <3>5, <3>6
    <2> QED BY <2>1 DEF PrepareCarriesHigherSafeQc
  <1> QED BY <1>1

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
      BY <1>1, UnchangedProvenanceVarsPreservesReducerProvenance
         DEF StrongInductiveInvariant, BeginLocalProposal, ProvenanceVars
    <2> QED BY <1>1, <2>4, <2>5, <2>6,
                  UnchangedLineageVarsPreservesLineageInvariant
       DEF StrongInductiveInvariant, BeginLocalProposal, LineageVars
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
      BY <1>1, UnchangedProvenanceVarsPreservesReducerProvenance
         DEF StrongInductiveInvariant, PersistProposal, ProvenanceVars
    <2> QED BY <1>1, <2>5, <2>6, <2>7,
                  UnchangedLineageVarsPreservesLineageInvariant
       DEF StrongInductiveInvariant, PersistProposal, LineageVars
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
      BY <1>1, UnchangedProvenanceVarsPreservesReducerProvenance
         DEF StrongInductiveInvariant, CompleteProposalSignature,
             ProvenanceVars
    <2> QED BY <1>1, <2>2, <2>3, <2>4,
                  UnchangedLineageVarsPreservesLineageInvariant
       DEF StrongInductiveInvariant, CompleteProposalSignature, LineageVars
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
      BY <1>1, UnchangedProvenanceVarsPreservesReducerProvenance, Isa
         DEF StrongInductiveInvariant, ResumeProposal,
             ProvenanceVars,
             ContextIdentityBindsFrozenEpoch,
             OldContextCertificateRejected, ContextParentWasApplied,
             QcValid, CurrentEpoch
    <2> QED BY <1>1, <2>3, <2>4,
                  UnchangedLineageVarsPreservesLineageInvariant
       DEF StrongInductiveInvariant, ResumeProposal, LineageVars
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
      <3>1. PendingVoteWritesAuthorized
        BY <1>1 DEF StrongInductiveInvariant, ReducerProvenanceInvariant
      <3>2. PrepareCarriesHigherSafeQc(PrepareVoteFor(node, proposal))
        <4>1. /\ node \in ValidatorIds
              /\ TypeInvariant
              /\ LineageInvariant
              /\ HighestAndLockAreCertified
              /\ ProposalValidFor(node, proposal)
              /\ lockRank[node] < proposal.view
          <5>1. /\ node \in Honest
                /\ Honest \subseteq ValidatorIds
            BY <1>1
               DEF BeginPrepare, StrongInductiveInvariant, Safety,
                   TypeInvariant, ModelConfiguration,
                   QuorumConfiguration
          <5>2. /\ TypeInvariant
                /\ LineageInvariant
                /\ HighestAndLockAreCertified
            BY <1>1
               DEF StrongInductiveInvariant, Safety,
                   ReducerProvenanceInvariant
          <5>3. /\ ProposalValidFor(node, proposal)
                /\ lockRank[node] < proposal.view
            BY <1>1 DEF BeginPrepare
          <5> QED BY <5>1, <5>2, <5>3
        <4> QED BY <4>1, SafeProposalCarriesCommitLineage
      <3>3. /\ PrepareRequestFor(node, proposal).node \in Honest
            /\ PrepareRequestFor(node, proposal).vote.phase = "Prepare"
            /\ PrepareRequestFor(node, proposal).vote.signer =
                 PrepareRequestFor(node, proposal).node
            /\ PrepareRequestFor(node, proposal).vote.context = context
            /\ PrepareRequestFor(node, proposal).vote.view = nodeView[node]
            /\ PrepareRequestFor(node, proposal).vote.subject
                 \in ValidSubjects
            /\ BodyHeldBy(durableBodies, node, context, proposal.subject)
            /\ CanAppendVote(prepareIntents,
                             PrepareRequestFor(node, proposal).vote)
        BY <1>1, <3>2, SMT
           DEF BeginPrepare, PrepareRequestFor, PrepareVoteFor,
               PrepareWal, Vote, ProposalValidFor, Proposal,
               PrepareSignerAvailability, CanAppendVote, SameVoteSlot
      <3>4. /\ pendingPrepare' =
                     pendingPrepare \cup {PrepareRequestFor(node, proposal)}
            /\ pendingLockCommit' = pendingLockCommit
            /\ pendingTimeout' = pendingTimeout
            /\ prepareIntents' = prepareIntents
            /\ commitIntents' = commitIntents
            /\ timeoutIntents' = timeoutIntents
            /\ context' = context
            /\ nodeView' = nodeView
            /\ durableBodies' = durableBodies
            /\ prepareQCs' = prepareQCs
            /\ lockRank' = lockRank
            /\ lockSubject' = lockSubject
        BY <1>1 DEF BeginPrepare
      <3>5. \A pending \in pendingPrepare':
               /\ pending.node \in Honest
               /\ pending.vote.phase = "Prepare"
               /\ pending.vote.signer = pending.node
               /\ pending.vote.context = context'
               /\ pending.vote.view = nodeView'[pending.node]
               /\ pending.vote.subject \in ValidSubjects
               /\ BodyHeldBy(durableBodies', pending.node,
                             pending.vote.context, pending.vote.subject)
               /\ CanAppendVote(prepareIntents', pending.vote)
               /\ PrepareCarriesHigherSafeQc(pending.vote)'
        <4>1. ASSUME NEW pending \in pendingPrepare'
               PROVE /\ pending.node \in Honest
                     /\ pending.vote.phase = "Prepare"
                     /\ pending.vote.signer = pending.node
                     /\ pending.vote.context = context'
                     /\ pending.vote.view = nodeView'[pending.node]
                     /\ pending.vote.subject \in ValidSubjects
                     /\ BodyHeldBy(durableBodies', pending.node,
                                   pending.vote.context,
                                   pending.vote.subject)
                     /\ CanAppendVote(prepareIntents', pending.vote)
                     /\ PrepareCarriesHigherSafeQc(pending.vote)'
          <5>1. pending \in pendingPrepare
                  \/ pending = PrepareRequestFor(node, proposal)
            BY <3>4, <4>1
          <5>2. CASE pending \in pendingPrepare
            BY <3>1, <3>4, <4>1, <5>2, IsaM("blast")
               DEF PendingVoteWritesAuthorized,
                   PrepareCarriesHigherSafeQc
          <5>3. CASE pending = PrepareRequestFor(node, proposal)
            <6>1. PrepareCarriesHigherSafeQc(
                     PrepareRequestFor(node, proposal).vote)
              BY <3>2 DEF PrepareRequestFor, PrepareWal
            <6>2. (PrepareRequestFor(node, proposal).vote)' =
                     PrepareRequestFor(node, proposal).vote
              BY <3>4
                 DEF PrepareRequestFor, PrepareVoteFor, PrepareWal, Vote
            <6>3. PrepareCarriesHigherSafeQc(
                     PrepareRequestFor(node, proposal).vote)'
              BY <3>4, <6>1, <6>2 DEF PrepareCarriesHigherSafeQc
            <6>4. /\ pending.node =
                        PrepareRequestFor(node, proposal).node
                  /\ pending.vote =
                        PrepareRequestFor(node, proposal).vote
              BY <5>3
            <6>5. /\ pending.node = node
                  /\ pending.vote = PrepareVoteFor(node, proposal)
              BY <6>4 DEF PrepareRequestFor, PrepareWal
            <6>6. pending.node \in Honest
              BY <3>3, <6>4
            <6>7. pending.vote.phase = "Prepare"
              BY <3>3, <6>4
            <6>8. pending.vote.signer = pending.node
              BY <3>3, <6>4
            <6>9. pending.vote.context = context'
              BY <3>3, <3>4, <6>4
            <6>10. pending.vote.view = nodeView[node]
              BY <3>3, <6>4
            <6>11. nodeView'[pending.node] = nodeView[node]
              BY <3>4, <6>5
            <6>12. pending.vote.view = nodeView'[pending.node]
              BY <6>10, <6>11
            <6>13. pending.vote.subject \in ValidSubjects
              BY <3>3, <6>4
            <6>14. BodyHeldBy(durableBodies', pending.node,
                             pending.vote.context, pending.vote.subject)
              BY <3>3, <3>4, <6>5
                 DEF PrepareVoteFor, Vote
            <6>15. CanAppendVote(prepareIntents', pending.vote)
              BY <3>3, <3>4, <6>5
                 DEF PrepareRequestFor, PrepareWal
            <6>16. PrepareCarriesHigherSafeQc(pending.vote)'
              <7>1. ASSUME NEW commitVote \in commitIntents',
                            /\ pending.vote.signer \in Honest
                            /\ commitVote.signer = pending.vote.signer
                            /\ commitVote.context = pending.vote.context
                            /\ commitVote.phase = "Commit"
                            /\ commitVote.view < pending.vote.view
                            /\ commitVote.subject # pending.vote.subject
                     PROVE \E qc \in prepareQCs':
                             /\ qc.context = pending.vote.context
                             /\ qc.phase = "Prepare"
                             /\ commitVote.view < qc.view
                             /\ qc.view < pending.vote.view
                             /\ qc.subject = pending.vote.subject
                <8>1. commitVote \in commitIntents
                  BY <3>4, <7>1
                <8>2. /\ PrepareRequestFor(node, proposal).vote.signer
                              \in Honest
                      /\ commitVote.signer =
                           PrepareRequestFor(node, proposal).vote.signer
                      /\ commitVote.context =
                           PrepareRequestFor(node, proposal).vote.context
                      /\ commitVote.phase = "Commit"
                      /\ commitVote.view <
                           PrepareRequestFor(node, proposal).vote.view
                      /\ commitVote.subject #
                           PrepareRequestFor(node, proposal).vote.subject
                  BY <6>4, <7>1
                <8>3. \E qc \in prepareQCs:
                         /\ qc.context =
                              PrepareRequestFor(node, proposal).vote.context
                         /\ qc.phase = "Prepare"
                         /\ commitVote.view < qc.view
                         /\ qc.view <
                              PrepareRequestFor(node, proposal).vote.view
                         /\ qc.subject =
                              PrepareRequestFor(node, proposal).vote.subject
                  BY <6>1, <8>1, <8>2 DEF PrepareCarriesHigherSafeQc
                <8>4. PICK qc \in prepareQCs:
                         /\ qc.context =
                              PrepareRequestFor(node, proposal).vote.context
                         /\ qc.phase = "Prepare"
                         /\ commitVote.view < qc.view
                         /\ qc.view <
                              PrepareRequestFor(node, proposal).vote.view
                         /\ qc.subject =
                              PrepareRequestFor(node, proposal).vote.subject
                  BY <8>3
                <8>5. qc \in prepareQCs'
                  BY <3>4, <8>4
                <8>6. /\ qc.context = pending.vote.context
                      /\ qc.phase = "Prepare"
                      /\ commitVote.view < qc.view
                      /\ qc.view < pending.vote.view
                      /\ qc.subject = pending.vote.subject
                  BY <6>4, <8>4
                <8> QED BY <8>5, <8>6
              <7> QED BY <7>1 DEF PrepareCarriesHigherSafeQc
            <6> QED BY <6>6, <6>7, <6>8, <6>9, <6>12,
                         <6>13, <6>14, <6>15, <6>16
          <5> QED BY <5>1, <5>2, <5>3
        <4> QED BY <4>1
      <3>6. \A pending \in pendingLockCommit':
               /\ pending.node \in Honest
               /\ pending.vote.phase = "Commit"
               /\ pending.vote.signer = pending.node
               /\ pending.vote.context = context'
               /\ pending.vote.context = pending.qc.context
               /\ pending.vote.view = pending.qc.view
               /\ pending.vote.subject = pending.qc.subject
               /\ pending.qc.phase = "Prepare"
               /\ pending.qc \in prepareQCs'
               /\ pending.vote.view = nodeView'[pending.node]
               /\ ~NodeTimedOut(pending.node, pending.vote.view)'
               /\ pending.vote.subject \in ValidSubjects
               /\ BodyHeldBy(durableBodies', pending.node,
                             pending.vote.context, pending.vote.subject)
               /\ pending.qc.view >= lockRank'[pending.node]
               /\ (pending.qc.view = lockRank'[pending.node]
                     => pending.qc.subject = lockSubject'[pending.node])
               /\ CanAppendVote(commitIntents', pending.vote)
        BY <3>1, <3>4, IsaM("blast")
           DEF PendingVoteWritesAuthorized, NodeTimedOut
      <3>7. \A pending \in pendingTimeout':
               /\ pending.node \in Honest
               /\ pending.vote.signer = pending.node
               /\ pending.vote.context = context'
               /\ pending.vote.view = nodeView'[pending.node]
               /\ CanAppendTimeout(timeoutIntents', pending.vote)
               /\ TimeoutVoteProtectsCommitSet(pending.vote,
                                               commitIntents')
        <4>1. \A pending \in pendingTimeout:
                 /\ pending.node \in Honest
                 /\ pending.vote.signer = pending.node
                 /\ pending.vote.context = context
                 /\ pending.vote.view = nodeView[pending.node]
                 /\ CanAppendTimeout(timeoutIntents, pending.vote)
                 /\ TimeoutVoteProtectsCommitSet(pending.vote,
                                                 commitIntents)
          BY <3>1 DEF PendingVoteWritesAuthorized
        <4>2. /\ pendingTimeout' = pendingTimeout
              /\ timeoutIntents' = timeoutIntents
              /\ commitIntents' = commitIntents
              /\ context' = context
              /\ nodeView' = nodeView
          BY <1>1 DEF BeginPrepare
        <4> QED BY <4>1, <4>2
           DEF CanAppendTimeout, TimeoutVoteProtectsCommitSet
      <3> QED BY <3>5, <3>6, <3>7
         DEF PendingVoteWritesAuthorized, NodeTimedOut
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
             PendingCertificateWritesAuthorized, TCValid, HighRefValid,
             CurrentEpoch, CurrentVoters,
             HonestVoteTransportBacked, QcTransportBacked,
             HonestTimeoutTransportBacked, TcTransportBacked,
             CertificatesBackedByIntents, HonestDurableIntentsSound,
             FormedTimeoutCertificatesSound,
             DurableTimeoutsProtectCommits, HighestAndLockAreCertified,
             VoteIntentFor, TCValid, HighRefValid,
             CurrentEpoch, CurrentVoters
    <2>8. ReducerProvenanceInvariant'
      BY <2>6, <2>7
         DEF ReducerProvenanceInvariant,
             ReducerProvenanceWithoutVoteTransport
    <2> QED BY <1>1, <2>4, <2>5, <2>8,
                  UnchangedLineageVarsPreservesLineageInvariant
       DEF StrongInductiveInvariant, BeginPrepare, LineageVars
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

THEOREM DistinctSignerAppendPreservesCanAppendVote ==
  \A votes, appended, candidate:
    CanAppendVote(votes, candidate)
      /\ appended.signer # candidate.signer
      => CanAppendVote(votes \cup {appended}, candidate)
BY SMT DEF CanAppendVote, SameVoteSlot

THEOREM DistinctSignerAppendPreservesCanAppendTimeout ==
  \A votes, appended, candidate:
    CanAppendTimeout(votes, candidate)
      /\ appended.signer # candidate.signer
      => CanAppendTimeout(votes \cup {appended}, candidate)
BY SMT DEF CanAppendTimeout, SameTimeoutSlot

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
      <3>1. PendingVoteWritesAuthorized
        BY <1>1 DEF StrongInductiveInvariant, ReducerProvenanceInvariant
      <3>2. ASSUME NEW pending \in pendingPrepare'
             PROVE /\ pending.node \in Honest
                   /\ pending.vote.phase = "Prepare"
                   /\ pending.vote.signer = pending.node
                   /\ pending.vote.context = context'
                   /\ pending.vote.view = nodeView'[pending.node]
                   /\ pending.vote.subject \in ValidSubjects
                   /\ BodyHeldBy(durableBodies', pending.node,
                                 pending.vote.context,
                                 pending.vote.subject)
                   /\ CanAppendVote(prepareIntents', pending.vote)
                   /\ PrepareCarriesHigherSafeQc(pending.vote)'
        <4>1. /\ pending \in pendingPrepare
              /\ pending # request
          BY <1>1, <3>2 DEF PersistPrepare
        <4>2. /\ pending \in AllPendingRequests
              /\ request \in AllPendingRequests
              /\ RequestsUniqueByNode(AllPendingRequests)
          BY <1>1, <4>1
             DEF StrongInductiveInvariant, Safety,
                 OnePendingPersistencePerNode, AllPendingRequests,
                 PersistPrepare
        <4>3. pending.node # request.node
          BY <4>1, <4>2 DEF RequestsUniqueByNode
        <4>4. /\ CanAppendVote(prepareIntents, pending.vote)
              /\ request.vote.signer # pending.vote.signer
          BY <2>1, <3>1, <4>1, <4>3
             DEF PendingVoteWritesAuthorized
        <4>5. CanAppendVote(prepareIntents \cup {request.vote},
                            pending.vote)
          BY <4>4, DistinctSignerAppendPreservesCanAppendVote
        <4> QED BY <1>1, <3>1, <4>1, <4>5, Isa
           DEF PendingVoteWritesAuthorized, PersistPrepare,
               PrepareCarriesHigherSafeQc
      <3>3. /\ \A pending \in pendingLockCommit':
                     /\ pending.node \in Honest
                     /\ pending.vote.phase = "Commit"
                     /\ pending.vote.signer = pending.node
                     /\ pending.vote.context = context'
                     /\ pending.vote.context = pending.qc.context
                     /\ pending.vote.view = pending.qc.view
                     /\ pending.vote.subject = pending.qc.subject
                     /\ pending.qc.phase = "Prepare"
                     /\ pending.qc \in prepareQCs'
                     /\ pending.vote.view = nodeView'[pending.node]
                     /\ ~NodeTimedOut(pending.node, pending.vote.view)'
                     /\ pending.vote.subject \in ValidSubjects
                     /\ BodyHeldBy(durableBodies', pending.node,
                                   pending.vote.context,
                                   pending.vote.subject)
                     /\ pending.qc.view >= lockRank'[pending.node]
                     /\ (pending.qc.view = lockRank'[pending.node]
                           => pending.qc.subject = lockSubject'[pending.node])
                     /\ CanAppendVote(commitIntents', pending.vote)
            /\ \A pending \in pendingTimeout':
                     /\ pending.node \in Honest
                     /\ pending.vote.signer = pending.node
                     /\ pending.vote.context = context'
                     /\ pending.vote.view = nodeView'[pending.node]
                     /\ CanAppendTimeout(timeoutIntents', pending.vote)
                     /\ TimeoutVoteProtectsCommitSet(pending.vote,
                                                     commitIntents')
        BY <1>1, <3>1, Isa
           DEF PersistPrepare, PendingVoteWritesAuthorized, NodeTimedOut
      <3> QED BY <3>2, <3>3
         DEF PendingVoteWritesAuthorized, NodeTimedOut
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
             PendingCertificateWritesAuthorized, TCValid, HighRefValid,
             CurrentEpoch, CurrentVoters, QcTransportBacked,
             HonestTimeoutTransportBacked, TcTransportBacked,
             FormedTimeoutCertificatesSound,
             DurableTimeoutsProtectCommits, HighestAndLockAreCertified,
             TCValid, HighRefValid, CurrentEpoch, CurrentVoters
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
    <2>18. LineageInvariant'
      <3>1. /\ TypeInvariant
            /\ LineageInvariant
            /\ PendingVoteWritesAuthorized
            /\ PersistPrepare(request)
        BY <1>1
           DEF StrongInductiveInvariant, Safety,
               ReducerProvenanceInvariant
      <3> QED BY <3>1, PersistPreparePreservesLineageInvariant
    <2> QED BY <2>11, <2>12, <2>17, <2>18
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
    <2>7. ReducerProvenanceWithoutVoteTransport'
      BY <1>1, UnchangedVoteIndependentProvenancePreserves
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             ReducerProvenanceWithoutVoteTransport,
             CompleteVoteSignature, ProvenanceWithoutVoteTransportVars
    <2>8. ReducerProvenanceInvariant'
      BY <2>6, <2>7
         DEF ReducerProvenanceInvariant,
             ReducerProvenanceWithoutVoteTransport
    <2> QED BY <1>1, <2>4, <2>5, <2>8,
                  UnchangedLineageVarsPreservesLineageInvariant
       DEF StrongInductiveInvariant, CompleteVoteSignature, LineageVars
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
        <4>1. ASSUME NEW envelope \in voteNetwork',
                      envelope.vote.signer \in Honest
               PROVE VoteIntentFor(envelope.vote)'
          <5>1. \/ envelope \in voteNetwork
                \/ envelope \in BroadcastVotes(
                     Vote(context, roundView, phase, subject, signer))
            BY <1>1, <4>1 DEF ByzantineBroadcastVote
          <5>2. CASE envelope \in voteNetwork
            BY <1>1, <3>1, <4>1, <5>2
               DEF ByzantineBroadcastVote, VoteIntentFor
          <5>3. CASE envelope \in BroadcastVotes(
                            Vote(context, roundView, phase, subject, signer))
            BY <3>2, <4>1, <5>3
          <5> QED BY <5>1, <5>2, <5>3
        <4> QED BY <4>1
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
    <2>4. ReducerProvenanceWithoutVoteTransport'
      BY <1>1, UnchangedVoteIndependentProvenancePreserves
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             ReducerProvenanceWithoutVoteTransport,
             ByzantineBroadcastVote, ProvenanceWithoutVoteTransportVars
    <2>5. ReducerProvenanceInvariant'
      BY <2>2, <2>4
         DEF ReducerProvenanceInvariant,
             ReducerProvenanceWithoutVoteTransport
    <2> QED BY <1>1, <2>3, <2>5,
                  UnchangedLineageVarsPreservesLineageInvariant
       DEF StrongInductiveInvariant, ByzantineBroadcastVote, LineageVars
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
    <2>3. ReducerProvenanceWithoutVoteTransport'
      BY <1>1, UnchangedVoteIndependentProvenancePreserves
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             ReducerProvenanceWithoutVoteTransport, DeliverVote,
             ProvenanceWithoutVoteTransportVars
    <2>4. ReducerProvenanceInvariant'
      BY <2>1, <2>3
         DEF ReducerProvenanceInvariant,
             ReducerProvenanceWithoutVoteTransport
    <2> QED BY <1>1, <2>2, <2>4,
                  UnchangedLineageVarsPreservesLineageInvariant
       DEF StrongInductiveInvariant, DeliverVote, LineageVars
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
    <2>3. Safety'
      BY <1>1, <2>1, <2>2, Isa
         DEF StrongInductiveInvariant, Safety, ResumeVote,
             TypeInvariant, OnePendingPersistencePerNode,
             AllPendingRequests, RequestsUniqueByNode,
             ProposalSigningRequiresIntent, TimeoutSigningRequiresIntent,
             HonestPrepareUniqueness, HonestCommitUniqueness,
             HonestTimeoutUniqueness, LockBelowHighest, DecisionAgreement,
             AppliedRequiresDecision
    <2>4. /\ ContextIdentityBindsFrozenEpoch'
          /\ OldContextCertificateRejected'
          /\ ContextParentWasApplied'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, ResumeVote,
             ContextIdentityBindsFrozenEpoch,
             OldContextCertificateRejected, ContextParentWasApplied,
             QcValid, CurrentEpoch
    <2>5. ReducerProvenanceInvariant'
      BY <1>1, UnchangedProvenanceVarsPreservesReducerProvenance
         DEF StrongInductiveInvariant, ResumeVote, ProvenanceVars
    <2> QED BY <1>1, <2>3, <2>4, <2>5,
                  UnchangedLineageVarsPreservesLineageInvariant
       DEF StrongInductiveInvariant, ResumeVote, LineageVars
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
        <4>1. ASSUME NEW qc \in prepareQCs'
               PROVE /\ HistoricalQcValid(qc)
                     /\ CertificateBackedBy(qc.context.epoch, qc,
                                            prepareIntents')
          <5>1. qc \in prepareQCs \/ qc = NewQc
            BY <3>1, <4>1
          <5>2. CASE qc \in prepareQCs
            BY <3>1, <3>2, <4>1, <5>2
          <5>3. CASE qc = NewQc
            BY <1>1, <2>2, <3>1, <5>3
               DEF CurrentEpoch, QC
          <5> QED BY <5>1, <5>2, <5>3
        <4> QED BY <4>1
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
             FormPrepareQC, NewQc, PendingCertificateWritesAuthorized,
             TCValid, HighRefValid, CurrentEpoch, CurrentVoters
    <2>10. HighestAndLockAreCertified'
      BY <1>1, SMT
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             FormPrepareQC, NewQc, HighestAndLockAreCertified
    <2>11. /\ HonestVoteUnique(prepareIntents)'
          /\ HonestVoteUnique(commitIntents)'
          /\ HonestTimeoutUnique(timeoutIntents)'
          /\ IntentPhasesCorrect'
          /\ HonestVoteTransportBacked'
          /\ HonestTimeoutTransportBacked'
          /\ TcTransportBacked'
          /\ HonestDurableIntentsSound'
          /\ FormedTimeoutCertificatesSound'
          /\ DurableTimeoutsProtectCommits'
      <3>1. /\ height' = height
            /\ context' = context
            /\ prepareIntents' = prepareIntents
            /\ commitIntents' = commitIntents
            /\ timeoutIntents' = timeoutIntents
            /\ height' = height
            /\ height' = height
            /\ context' = context
            /\ prepareQCs' = prepareQCs \cup {NewQc}
            /\ durableBodies' = durableBodies
            /\ receivedVotes' = receivedVotes
            /\ receivedTimeoutVotes' = receivedTimeoutVotes
            /\ receivedTCs' = receivedTCs
            /\ formedTCs' = formedTCs
            /\ installedTCs' = installedTCs
            /\ voteNetwork' = voteNetwork
            /\ timeoutNetwork' = timeoutNetwork
            /\ tcNetwork' = tcNetwork
        BY <1>1 DEF FormPrepareQC
      <3>2. /\ HonestVoteUnique(prepareIntents)'
            /\ HonestVoteUnique(commitIntents)'
            /\ HonestTimeoutUnique(timeoutIntents)'
            /\ IntentPhasesCorrect'
        BY <1>1, <3>1, SMT
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               HonestVoteUnique, HonestTimeoutUnique, IntentPhasesCorrect
      <3>3. /\ HonestVoteTransportBacked'
            /\ HonestTimeoutTransportBacked'
            /\ TcTransportBacked'
        BY <1>1, <3>1, SMT
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               HonestVoteTransportBacked, HonestTimeoutTransportBacked,
               TcTransportBacked, VoteIntentFor, TCValid, HighRefValid,
               CurrentEpoch, CurrentVoters
      <3>4. /\ HonestDurableIntentsSound'
            /\ FormedTimeoutCertificatesSound'
            /\ DurableTimeoutsProtectCommits'
        BY <1>1, <3>1, SMT
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               HonestDurableIntentsSound, FormedTimeoutCertificatesSound,
               DurableTimeoutsProtectCommits
      <3> QED BY <3>2, <3>3, <3>4
    <2>12. PendingVoteWritesAuthorized'
      BY <1>1, SMT
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             FormPrepareQC, PendingVoteWritesAuthorized,
             PrepareCarriesHigherSafeQc, NodeTimedOut
    <2>13. ReducerProvenanceInvariant'
      BY <2>7, <2>8, <2>9, <2>10, <2>11, <2>12
         DEF ReducerProvenanceInvariant
    <2> QED BY <1>1, <2>5, <2>6, <2>13,
                  FormPrepareQCPreservesLineageInvariant
       DEF StrongInductiveInvariant
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
      <3>1. TypeInvariant'
        BY <1>1, Isa
           DEF StrongInductiveInvariant, Safety, TypeInvariant, DeliverQC
      <3>2. OnePendingPersistencePerNode'
        BY <1>1, Isa
           DEF StrongInductiveInvariant, Safety,
               OnePendingPersistencePerNode, RequestsUniqueByNode,
               AllPendingRequests, DeliverQC
      <3>3. /\ ProposalSigningRequiresIntent'
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
           DEF StrongInductiveInvariant, Safety, DeliverQC,
               ProposalSigningRequiresIntent, PrepareSigningRequiresIntent,
               CommitSigningRequiresIntent, TimeoutSigningRequiresIntent,
               HonestPrepareUniqueness, HonestCommitUniqueness,
               HonestTimeoutUniqueness, LockBelowHighest, DecisionAgreement,
               AppliedRequiresDecision
      <3>4. Safety'
        BY <3>1, <3>2, <3>3 DEF Safety
      <3>5. /\ ContextIdentityBindsFrozenEpoch'
            /\ OldContextCertificateRejected'
            /\ ContextParentWasApplied'
        BY <1>1, Isa
           DEF StrongInductiveInvariant, DeliverQC,
               ContextIdentityBindsFrozenEpoch, OldContextCertificateRejected,
               ContextParentWasApplied, QcValid, CurrentEpoch
      <3> QED BY <3>4, <3>5
    <2>3. /\ HonestVoteUnique(prepareIntents)'
          /\ HonestVoteUnique(commitIntents)'
          /\ HonestTimeoutUnique(timeoutIntents)'
          /\ IntentPhasesCorrect'
          /\ PendingVoteWritesAuthorized'
          /\ PendingCertificateWritesAuthorized'
      <3>1. /\ prepareIntents' = prepareIntents
            /\ commitIntents' = commitIntents
            /\ timeoutIntents' = timeoutIntents
            /\ height' = height
            /\ context' = context
            /\ prepareQCs' = prepareQCs
            /\ pendingPrepare' = pendingPrepare
            /\ pendingLockCommit' = pendingLockCommit
            /\ pendingTimeout' = pendingTimeout
            /\ pendingObservePrepare' = pendingObservePrepare
            /\ pendingInstallTC' = pendingInstallTC
            /\ pendingDecision' = pendingDecision
            /\ context' = context
            /\ nodeView' = nodeView
            /\ durableBodies' = durableBodies
            /\ prepareQCs' = prepareQCs
            /\ commitQCs' = commitQCs
            /\ formedTCs' = formedTCs
            /\ lockRank' = lockRank
            /\ lockSubject' = lockSubject
            /\ highestRank' = highestRank
            /\ highestSubject' = highestSubject
        BY <1>1 DEF DeliverQC
      <3>2. /\ HonestVoteUnique(prepareIntents)'
            /\ HonestVoteUnique(commitIntents)'
            /\ HonestTimeoutUnique(timeoutIntents)'
            /\ IntentPhasesCorrect'
        BY <1>1, <3>1, SMT
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               HonestVoteUnique, HonestTimeoutUnique, IntentPhasesCorrect
      <3>3. PendingVoteWritesAuthorized'
        BY <1>1, <3>1, SMT
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               PendingVoteWritesAuthorized, PrepareCarriesHigherSafeQc,
               NodeTimedOut
      <3>4. PendingCertificateWritesAuthorized'
        BY <1>1, <3>1, SMT
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               PendingCertificateWritesAuthorized, TCValid, HighRefValid,
               CurrentEpoch, CurrentVoters
      <3> QED BY <3>2, <3>3, <3>4
    <2>4. /\ HonestVoteTransportBacked'
          /\ HonestTimeoutTransportBacked'
          /\ TcTransportBacked'
      <3>1. /\ prepareIntents' = prepareIntents
            /\ commitIntents' = commitIntents
            /\ timeoutIntents' = timeoutIntents
            /\ height' = height
            /\ context' = context
            /\ prepareQCs' = prepareQCs
            /\ receivedVotes' = receivedVotes
            /\ receivedTimeoutVotes' = receivedTimeoutVotes
            /\ receivedTCs' = receivedTCs
            /\ installedTCs' = installedTCs
            /\ formedTCs' = formedTCs
            /\ voteNetwork' = voteNetwork
            /\ timeoutNetwork' = timeoutNetwork
            /\ tcNetwork' = tcNetwork
        BY <1>1 DEF DeliverQC
      <3>2. HonestVoteTransportBacked'
        BY <1>1, <3>1, Isa
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               HonestVoteTransportBacked, VoteIntentFor
      <3>3. HonestTimeoutTransportBacked'
        BY <1>1, <3>1, Isa
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               HonestTimeoutTransportBacked
      <3>4. TcTransportBacked'
        BY <1>1, <3>1, Isa
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               TcTransportBacked, TCValid, HighRefValid,
               CurrentEpoch, CurrentVoters
      <3> QED BY <3>2, <3>3, <3>4
    <2>5. /\ CertificatesBackedByIntents'
          /\ HonestDurableIntentsSound'
          /\ FormedTimeoutCertificatesSound'
          /\ DurableTimeoutsProtectCommits'
          /\ HighestAndLockAreCertified'
      BY <1>1, SMT
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             DeliverQC, CertificatesBackedByIntents,
             HonestDurableIntentsSound, FormedTimeoutCertificatesSound,
             DurableTimeoutsProtectCommits, HighestAndLockAreCertified
    <2>6. ReducerProvenanceInvariant'
      BY <2>1, <2>3, <2>4, <2>5 DEF ReducerProvenanceInvariant
    <2> QED BY <1>1, <2>2, <2>6,
                  UnchangedLineageVarsPreservesLineageInvariant
       DEF StrongInductiveInvariant, DeliverQC, LineageVars
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
      <3>1. /\ generation \in [ValidatorIds -> Generations]
            /\ node \in ValidatorIds
            /\ generation[node] < MaxGeneration
            /\ MaxGeneration \in Nat
        BY <1>1
           DEF StrongInductiveInvariant, Safety, TypeInvariant,
               Restart, ModelConfiguration
      <3>2. generation[node] + 1 \in Generations
        BY <3>1, SMT DEF Generations
      <3>3. generation' \in [ValidatorIds -> Generations]
        BY <1>1, <3>1, <3>2, Isa DEF Restart
      <3> QED BY <1>1, <3>3, Isa
         DEF StrongInductiveInvariant, Safety, TypeInvariant, Restart
    <2>2. /\ ProposalSigningRequiresIntent'
          /\ PrepareSigningRequiresIntent'
          /\ CommitSigningRequiresIntent'
          /\ TimeoutSigningRequiresIntent'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, Safety, Restart,
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
         DEF StrongInductiveInvariant, Safety, Restart,
             OnePendingPersistencePerNode, AllPendingRequests,
             RequestsUniqueByNode, ProposalSigningRequiresIntent,
             PrepareSigningRequiresIntent, CommitSigningRequiresIntent,
             TimeoutSigningRequiresIntent, HonestPrepareUniqueness,
             HonestCommitUniqueness, HonestTimeoutUniqueness,
             LockBelowHighest, DecisionAgreement, AppliedRequiresDecision
    <2>4. Safety'
      BY <2>1, <2>2, <2>3 DEF Safety
    <2>5. /\ ContextIdentityBindsFrozenEpoch'
          /\ OldContextCertificateRejected'
          /\ ContextParentWasApplied'
          /\ ReducerProvenanceInvariant'
      BY <1>1, UnchangedProvenanceVarsPreservesReducerProvenance, Isa
         DEF StrongInductiveInvariant, Restart,
             ProvenanceVars,
             ContextIdentityBindsFrozenEpoch,
             OldContextCertificateRejected, ContextParentWasApplied,
             QcValid, CurrentEpoch
    <2> QED BY <1>1, <2>4, <2>5,
                  UnchangedLineageVarsPreservesLineageInvariant
       DEF StrongInductiveInvariant, Restart, LineageVars
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
      BY <1>1, IsaT(120)
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
      <3>1. /\ pendingPrepare' \subseteq pendingPrepare
            /\ pendingLockCommit' \subseteq pendingLockCommit
            /\ pendingTimeout' \subseteq pendingTimeout
            /\ pendingObservePrepare' \subseteq pendingObservePrepare
            /\ pendingInstallTC' \subseteq pendingInstallTC
            /\ pendingDecision' \subseteq pendingDecision
        BY <1>1, Isa DEF Crash
      <3>2. /\ PendingVoteWritesAuthorized'
            /\ PendingCertificateWritesAuthorized'
        <4>1. /\ height' = height
              /\ context' = context
              /\ nodeView' = nodeView
              /\ durableBodies' = durableBodies
              /\ prepareIntents' = prepareIntents
              /\ commitIntents' = commitIntents
              /\ timeoutIntents' = timeoutIntents
              /\ prepareQCs' = prepareQCs
              /\ commitQCs' = commitQCs
              /\ formedTCs' = formedTCs
              /\ lockRank' = lockRank
              /\ lockSubject' = lockSubject
              /\ highestRank' = highestRank
              /\ highestSubject' = highestSubject
          BY <1>1 DEF Crash
        <4>2. PendingVoteWritesAuthorized'
          BY <1>1, <3>1, <4>1, SMT
             DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
                 PendingVoteWritesAuthorized, PrepareCarriesHigherSafeQc,
                 NodeTimedOut
        <4>3. PendingCertificateWritesAuthorized'
          BY <1>1, <3>1, <4>1, SMT
             DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
                 PendingCertificateWritesAuthorized, TCValid, HighRefValid,
                 CurrentEpoch, CurrentVoters
        <4> QED BY <4>2, <4>3
      <3>3. /\ HonestVoteUnique(prepareIntents)'
            /\ HonestVoteUnique(commitIntents)'
            /\ HonestTimeoutUnique(timeoutIntents)'
            /\ IntentPhasesCorrect'
            /\ HonestVoteTransportBacked'
            /\ QcTransportBacked'
            /\ HonestTimeoutTransportBacked'
            /\ TcTransportBacked'
            /\ CertificatesBackedByIntents'
            /\ HonestDurableIntentsSound'
            /\ FormedTimeoutCertificatesSound'
            /\ DurableTimeoutsProtectCommits'
            /\ HighestAndLockAreCertified'
        BY <1>1, SMT
           DEF StrongInductiveInvariant, Crash,
               ReducerProvenanceInvariant, HonestVoteUnique,
               HonestTimeoutUnique, IntentPhasesCorrect,
               HonestVoteTransportBacked, QcTransportBacked,
               HonestTimeoutTransportBacked, TcTransportBacked,
               CertificatesBackedByIntents, HonestDurableIntentsSound,
               FormedTimeoutCertificatesSound,
               DurableTimeoutsProtectCommits, HighestAndLockAreCertified,
               VoteIntentFor, TCValid, HighRefValid,
               CurrentEpoch, CurrentVoters
      <3> QED BY <3>2, <3>3 DEF ReducerProvenanceInvariant
    <2> QED BY <1>1, <2>4, <2>5, <2>6,
                  UnchangedLineageVarsPreservesLineageInvariant
       DEF StrongInductiveInvariant, Crash, LineageVars
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
      <3>1. PendingVoteWritesAuthorized
        BY <1>1 DEF StrongInductiveInvariant, ReducerProvenanceInvariant
      <3>2. /\ TimeoutRequestFor(node).node \in Honest
            /\ TimeoutRequestFor(node).vote.signer =
                 TimeoutRequestFor(node).node
            /\ TimeoutRequestFor(node).vote.context = context
            /\ TimeoutRequestFor(node).vote.view = nodeView[node]
            /\ CanAppendTimeout(timeoutIntents,
                                TimeoutRequestFor(node).vote)
            /\ TimeoutVoteProtectsCommitSet(
                 TimeoutRequestFor(node).vote, commitIntents)
        <4>1. node \in Honest
          BY <1>1
             DEF BeginTimeout, TimeoutRequestFor,
                 LocalTimeoutVoteFor, TimeoutWal
        <4>2. /\ TimeoutRequestFor(node).node = node
              /\ TimeoutRequestFor(node).vote = LocalTimeoutVoteFor(node)
              /\ LocalTimeoutVoteFor(node).signer = node
              /\ LocalTimeoutVoteFor(node).context = context
              /\ LocalTimeoutVoteFor(node).view = nodeView[node]
          BY DEF TimeoutRequestFor, TimeoutWal,
                 LocalTimeoutVoteFor, TimeoutVote
        <4>3. TimeoutVoteProtectsCommitSet(
                 LocalTimeoutVoteFor(node), commitIntents)
          BY <1>1 DEF BeginTimeout
        <4>4. \A prior \in timeoutIntents:
                 ~SameTimeoutSlot(prior, LocalTimeoutVoteFor(node))
          <5>1. ASSUME NEW prior \in timeoutIntents
                 PROVE ~SameTimeoutSlot(
                           prior, LocalTimeoutVoteFor(node))
            <6>1. ASSUME SameTimeoutSlot(
                           prior, LocalTimeoutVoteFor(node))
                   PROVE FALSE
              <7>1. NodeTimedOut(node, nodeView[node])
                BY <5>1, <6>1
                   DEF NodeTimedOut, SameTimeoutSlot,
                       LocalTimeoutVoteFor, TimeoutVote
              <7>2. ~NodeTimedOut(node, nodeView[node])
                BY <1>1 DEF BeginTimeout
              <7> QED BY <7>1, <7>2
            <6> QED BY <6>1
          <5> QED BY <5>1
        <4>5. CanAppendTimeout(timeoutIntents,
                              LocalTimeoutVoteFor(node))
          BY <4>1, <4>4, SMT
             DEF CanAppendTimeout, LocalTimeoutVoteFor, TimeoutVote
        <4> QED BY <4>1, <4>2, <4>3, <4>5
      <3>3. /\ pendingTimeout' =
                     pendingTimeout \cup {TimeoutRequestFor(node)}
            /\ pendingPrepare' = pendingPrepare
            /\ pendingLockCommit' = pendingLockCommit
            /\ context' = context
            /\ nodeView' = nodeView
            /\ prepareIntents' = prepareIntents
            /\ commitIntents' = commitIntents
            /\ timeoutIntents' = timeoutIntents
            /\ durableBodies' = durableBodies
            /\ prepareQCs' = prepareQCs
            /\ lockRank' = lockRank
            /\ lockSubject' = lockSubject
        BY <1>1 DEF BeginTimeout
      <3>4. \A pending \in pendingPrepare':
               /\ pending.node \in Honest
               /\ pending.vote.phase = "Prepare"
               /\ pending.vote.signer = pending.node
               /\ pending.vote.context = context'
               /\ pending.vote.view = nodeView'[pending.node]
               /\ pending.vote.subject \in ValidSubjects
               /\ BodyHeldBy(durableBodies', pending.node,
                             pending.vote.context, pending.vote.subject)
               /\ CanAppendVote(prepareIntents', pending.vote)
               /\ PrepareCarriesHigherSafeQc(pending.vote)'
        BY <3>1, <3>3, SMT DEF PendingVoteWritesAuthorized,
                                    PrepareCarriesHigherSafeQc
      <3>5. \A pending \in pendingLockCommit':
               /\ pending.node \in Honest
               /\ pending.vote.phase = "Commit"
               /\ pending.vote.signer = pending.node
               /\ pending.vote.context = context'
               /\ pending.vote.context = pending.qc.context
               /\ pending.vote.view = pending.qc.view
               /\ pending.vote.subject = pending.qc.subject
               /\ pending.qc.phase = "Prepare"
               /\ pending.qc \in prepareQCs'
               /\ pending.vote.view = nodeView'[pending.node]
               /\ ~NodeTimedOut(pending.node, pending.vote.view)'
               /\ pending.vote.subject \in ValidSubjects
               /\ BodyHeldBy(durableBodies', pending.node,
                             pending.vote.context, pending.vote.subject)
               /\ pending.qc.view >= lockRank'[pending.node]
               /\ (pending.qc.view = lockRank'[pending.node]
                     => pending.qc.subject = lockSubject'[pending.node])
               /\ CanAppendVote(commitIntents', pending.vote)
        BY <3>1, <3>3, Isa
           DEF PendingVoteWritesAuthorized, NodeTimedOut
      <3>6. \A pending \in pendingTimeout':
               /\ pending.node \in Honest
               /\ pending.vote.signer = pending.node
               /\ pending.vote.context = context'
               /\ pending.vote.view = nodeView'[pending.node]
               /\ CanAppendTimeout(timeoutIntents', pending.vote)
               /\ TimeoutVoteProtectsCommitSet(pending.vote,
                                               commitIntents')
        <4>1. ASSUME NEW pending \in pendingTimeout'
               PROVE /\ pending.node \in Honest
                     /\ pending.vote.signer = pending.node
                     /\ pending.vote.context = context'
                     /\ pending.vote.view = nodeView'[pending.node]
                     /\ CanAppendTimeout(timeoutIntents', pending.vote)
                     /\ TimeoutVoteProtectsCommitSet(pending.vote,
                                                     commitIntents')
          <5>1. pending \in pendingTimeout
                  \/ pending = TimeoutRequestFor(node)
            BY <3>3, <4>1
          <5>2. CASE pending \in pendingTimeout
            BY <3>1, <3>3, <4>1, <5>2, Isa
               DEF PendingVoteWritesAuthorized
          <5>3. CASE pending = TimeoutRequestFor(node)
            <6>1. /\ pending.node = node
                  /\ pending.vote = LocalTimeoutVoteFor(node)
              BY <5>3 DEF TimeoutRequestFor, TimeoutWal
            <6>2. /\ node \in Honest
                  /\ LocalTimeoutVoteFor(node).signer = node
                  /\ LocalTimeoutVoteFor(node).context = context
                  /\ LocalTimeoutVoteFor(node).view = nodeView[node]
                  /\ CanAppendTimeout(timeoutIntents,
                                      LocalTimeoutVoteFor(node))
                  /\ TimeoutVoteProtectsCommitSet(
                       LocalTimeoutVoteFor(node), commitIntents)
              BY <3>2
                 DEF TimeoutRequestFor, TimeoutWal,
                     LocalTimeoutVoteFor, TimeoutVote
            <6>3. /\ context' = context
                  /\ nodeView' = nodeView
                  /\ timeoutIntents' = timeoutIntents
                  /\ commitIntents' = commitIntents
              BY <3>3
            <6>4. /\ pending.node \in Honest
                  /\ pending.vote.signer = pending.node
                  /\ pending.vote.context = context
                  /\ pending.vote.view = nodeView[pending.node]
                  /\ CanAppendTimeout(timeoutIntents, pending.vote)
                  /\ TimeoutVoteProtectsCommitSet(pending.vote,
                                                  commitIntents)
              BY <6>1, <6>2, Isa
            <6>5. /\ pending.vote.context = context'
                  /\ pending.vote.view = nodeView'[pending.node]
                  /\ CanAppendTimeout(timeoutIntents', pending.vote)
                  /\ TimeoutVoteProtectsCommitSet(pending.vote,
                                                  commitIntents')
              BY <6>3, <6>4, Isa
            <6> QED BY <6>4, <6>5
          <5> QED BY <5>1, <5>2, <5>3
        <4> QED BY <4>1
      <3> QED BY <3>4, <3>5, <3>6
         DEF PendingVoteWritesAuthorized, NodeTimedOut
    <2>6. /\ HonestVoteUnique(prepareIntents)'
          /\ HonestVoteUnique(commitIntents)'
          /\ HonestTimeoutUnique(timeoutIntents)'
          /\ IntentPhasesCorrect'
          /\ PendingCertificateWritesAuthorized'
      BY <1>1, SMT
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             BeginTimeout, HonestVoteUnique, HonestTimeoutUnique,
             IntentPhasesCorrect, PendingCertificateWritesAuthorized,
             TCValid, HighRefValid, CurrentEpoch, CurrentVoters
    <2>7. /\ HonestVoteTransportBacked'
          /\ QcTransportBacked'
          /\ HonestTimeoutTransportBacked'
          /\ TcTransportBacked'
      <3>1. /\ height' = height
            /\ context' = context
            /\ prepareIntents' = prepareIntents
            /\ commitIntents' = commitIntents
            /\ timeoutIntents' = timeoutIntents
            /\ prepareQCs' = prepareQCs
            /\ commitQCs' = commitQCs
            /\ formedTCs' = formedTCs
            /\ installedTCs' = installedTCs
            /\ receivedVotes' = receivedVotes
            /\ receivedQCs' = receivedQCs
            /\ receivedTimeoutVotes' = receivedTimeoutVotes
            /\ receivedTCs' = receivedTCs
            /\ voteNetwork' = voteNetwork
            /\ qcNetwork' = qcNetwork
            /\ timeoutNetwork' = timeoutNetwork
            /\ tcNetwork' = tcNetwork
        BY <1>1 DEF BeginTimeout
      <3>2. HonestVoteTransportBacked'
        BY <1>1, <3>1, Isa
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               HonestVoteTransportBacked, VoteIntentFor
      <3>3. QcTransportBacked'
        BY <1>1, <3>1, Isa
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               QcTransportBacked
      <3>4. HonestTimeoutTransportBacked'
        BY <1>1, <3>1, Isa
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               HonestTimeoutTransportBacked
      <3>5. TcTransportBacked'
        BY <1>1, <3>1, Isa
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               TcTransportBacked, TCValid, HighRefValid,
               CurrentEpoch, CurrentVoters
      <3> QED BY <3>2, <3>3, <3>4, <3>5
    <2>8. /\ CertificatesBackedByIntents'
          /\ HonestDurableIntentsSound'
          /\ FormedTimeoutCertificatesSound'
          /\ DurableTimeoutsProtectCommits'
          /\ HighestAndLockAreCertified'
      BY <1>1, IsaMT("blast", 120)
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             BeginTimeout, CertificatesBackedByIntents,
             HonestDurableIntentsSound, FormedTimeoutCertificatesSound,
             DurableTimeoutsProtectCommits, HighestAndLockAreCertified
    <2>9. ReducerProvenanceInvariant'
      BY <2>5, <2>6, <2>7, <2>8 DEF ReducerProvenanceInvariant
    <2> QED BY <1>1, <2>3, <2>4, <2>9,
                  UnchangedLineageVarsPreservesLineageInvariant
       DEF StrongInductiveInvariant, BeginTimeout, LineageVars
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
          /\ request.vote.context = context
          /\ request.vote.view = nodeView[request.node]
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
      <3>1. PendingVoteWritesAuthorized
        BY <1>1 DEF StrongInductiveInvariant, ReducerProvenanceInvariant
      <3>2. ASSUME NEW pending \in pendingTimeout'
             PROVE /\ pending.node \in Honest
                   /\ pending.vote.signer = pending.node
                   /\ pending.vote.context = context'
                   /\ pending.vote.view = nodeView'[pending.node]
                   /\ CanAppendTimeout(timeoutIntents', pending.vote)
                   /\ TimeoutVoteProtectsCommitSet(pending.vote,
                                                   commitIntents')
        <4>1. /\ pending \in pendingTimeout
              /\ pending # request
          BY <1>1, <3>2 DEF PersistTimeout
        <4>2. /\ pending \in AllPendingRequests
              /\ request \in AllPendingRequests
              /\ RequestsUniqueByNode(AllPendingRequests)
          BY <1>1, <4>1
             DEF StrongInductiveInvariant, Safety,
                 OnePendingPersistencePerNode, AllPendingRequests,
                 PersistTimeout
        <4>3. pending.node # request.node
          BY <4>1, <4>2 DEF RequestsUniqueByNode
        <4>4. /\ CanAppendTimeout(timeoutIntents, pending.vote)
              /\ request.vote.signer # pending.vote.signer
          BY <2>1, <3>1, <4>1, <4>3
             DEF PendingVoteWritesAuthorized
        <4>5. CanAppendTimeout(timeoutIntents \cup {request.vote},
                               pending.vote)
          BY <4>4, DistinctSignerAppendPreservesCanAppendTimeout
        <4> QED BY <1>1, <3>1, <4>1, <4>5, Isa
           DEF PendingVoteWritesAuthorized, PersistTimeout
      <3>3. \A pending \in pendingPrepare':
                     /\ pending.node \in Honest
                     /\ pending.vote.phase = "Prepare"
                     /\ pending.vote.signer = pending.node
                     /\ pending.vote.context = context'
                     /\ pending.vote.view = nodeView'[pending.node]
                     /\ pending.vote.subject \in ValidSubjects
                     /\ BodyHeldBy(durableBodies', pending.node,
                                   pending.vote.context,
                                   pending.vote.subject)
                     /\ CanAppendVote(prepareIntents', pending.vote)
                     /\ PrepareCarriesHigherSafeQc(pending.vote)'
        BY <1>1, <3>1, SMT
           DEF PersistTimeout, PendingVoteWritesAuthorized,
               PrepareCarriesHigherSafeQc
      <3>4. \A pending \in pendingLockCommit':
                     /\ pending.node \in Honest
                     /\ pending.vote.phase = "Commit"
                     /\ pending.vote.signer = pending.node
                     /\ pending.vote.context = context'
                     /\ pending.vote.context = pending.qc.context
                     /\ pending.vote.view = pending.qc.view
                     /\ pending.vote.subject = pending.qc.subject
                     /\ pending.qc.phase = "Prepare"
                     /\ pending.qc \in prepareQCs'
                     /\ pending.vote.view = nodeView'[pending.node]
                     /\ ~NodeTimedOut(pending.node, pending.vote.view)'
                     /\ pending.vote.subject \in ValidSubjects
                     /\ BodyHeldBy(durableBodies', pending.node,
                                   pending.vote.context,
                                   pending.vote.subject)
                     /\ pending.qc.view >= lockRank'[pending.node]
                     /\ (pending.qc.view = lockRank'[pending.node]
                           => pending.qc.subject = lockSubject'[pending.node])
                     /\ CanAppendVote(commitIntents', pending.vote)
        <4>1. ASSUME NEW pending \in pendingLockCommit'
               PROVE /\ pending.node \in Honest
                     /\ pending.vote.phase = "Commit"
                     /\ pending.vote.signer = pending.node
                     /\ pending.vote.context = context'
                     /\ pending.vote.context = pending.qc.context
                     /\ pending.vote.view = pending.qc.view
                     /\ pending.vote.subject = pending.qc.subject
                     /\ pending.qc.phase = "Prepare"
                     /\ pending.qc \in prepareQCs'
                     /\ pending.vote.view = nodeView'[pending.node]
                     /\ ~NodeTimedOut(pending.node, pending.vote.view)'
                     /\ pending.vote.subject \in ValidSubjects
                     /\ BodyHeldBy(durableBodies', pending.node,
                                   pending.vote.context,
                                   pending.vote.subject)
                     /\ pending.qc.view >= lockRank'[pending.node]
                     /\ (pending.qc.view = lockRank'[pending.node]
                           => pending.qc.subject = lockSubject'[pending.node])
                     /\ CanAppendVote(commitIntents', pending.vote)
          <5>1. /\ pending \in pendingLockCommit
                /\ request \in pendingTimeout
                /\ pending \in AllPendingRequests
                /\ request \in AllPendingRequests
                /\ RequestsUniqueByNode(AllPendingRequests)
            BY <1>1, <4>1
               DEF StrongInductiveInvariant, Safety,
                   OnePendingPersistencePerNode, AllPendingRequests,
                   PersistTimeout
          <5>2. pending # request
            BY <1>1, <4>1
               DEF StrongInductiveInvariant, Safety, TypeInvariant,
                   PersistTimeout, LockCommitWalSet, TimeoutWalSet
          <5>3. pending.node # request.node
            BY <5>1, <5>2, DistinctUniqueRequestsHaveDistinctNodes
          <5>4. /\ pending.node \in Honest
                /\ pending.vote.phase = "Commit"
                /\ pending.vote.signer = pending.node
                /\ pending.vote.context = context
                /\ pending.vote.context = pending.qc.context
                /\ pending.vote.view = pending.qc.view
                /\ pending.vote.subject = pending.qc.subject
                /\ pending.qc.phase = "Prepare"
                /\ pending.qc \in prepareQCs
                /\ pending.vote.view = nodeView[pending.node]
                /\ ~NodeTimedOut(pending.node, pending.vote.view)
                /\ pending.vote.subject \in ValidSubjects
                /\ BodyHeldBy(durableBodies, pending.node,
                              pending.vote.context, pending.vote.subject)
                /\ pending.qc.view >= lockRank[pending.node]
                /\ (pending.qc.view = lockRank[pending.node]
                      => pending.qc.subject = lockSubject[pending.node])
                /\ CanAppendVote(commitIntents, pending.vote)
            BY <3>1, <5>1 DEF PendingVoteWritesAuthorized
          <5>5. /\ request.vote.signer = request.node
                /\ timeoutIntents' = timeoutIntents \cup {request.vote}
                /\ context' = context
                /\ nodeView' = nodeView
                /\ prepareQCs' = prepareQCs
                /\ durableBodies' = durableBodies
                /\ lockRank' = lockRank
                /\ lockSubject' = lockSubject
                /\ commitIntents' = commitIntents
            BY <1>1, <2>1 DEF PersistTimeout
          <5>6. ~NodeTimedOut(pending.node, pending.vote.view)'
            BY <5>3, <5>4, <5>5, Isa DEF NodeTimedOut
          <5> QED BY <5>4, <5>5, <5>6, Isa
        <4> QED BY <4>1
      <3> QED BY <3>2, <3>3, <3>4
         DEF PendingVoteWritesAuthorized, NodeTimedOut
    <2>12. /\ HonestVoteUnique(prepareIntents)'
           /\ HonestVoteUnique(commitIntents)'
           /\ IntentPhasesCorrect'
           /\ PendingCertificateWritesAuthorized'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             PersistTimeout, HonestVoteUnique, IntentPhasesCorrect,
             PendingCertificateWritesAuthorized, TCValid, HighRefValid,
             CurrentEpoch, CurrentVoters
    <2>13. /\ HonestVoteTransportBacked'
           /\ QcTransportBacked'
           /\ TcTransportBacked'
      <3>1. /\ height' = height
            /\ context' = context
            /\ prepareIntents' = prepareIntents
            /\ commitIntents' = commitIntents
            /\ prepareQCs' = prepareQCs
            /\ commitQCs' = commitQCs
            /\ formedTCs' = formedTCs
            /\ installedTCs' = installedTCs
            /\ receivedVotes' = receivedVotes
            /\ receivedQCs' = receivedQCs
            /\ receivedTCs' = receivedTCs
            /\ voteNetwork' = voteNetwork
            /\ qcNetwork' = qcNetwork
            /\ tcNetwork' = tcNetwork
        BY <1>1 DEF PersistTimeout
      <3>2. HonestVoteTransportBacked'
        BY <1>1, <3>1, Isa
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               HonestVoteTransportBacked, VoteIntentFor
      <3>3. QcTransportBacked'
        BY <1>1, <3>1, Isa
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               QcTransportBacked
      <3>4. TcTransportBacked'
        BY <1>1, <3>1, Isa
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               TcTransportBacked, TCValid, HighRefValid,
               CurrentEpoch, CurrentVoters
      <3> QED BY <3>2, <3>3, <3>4
    <2>14. /\ CertificatesBackedByIntents'
           /\ HonestDurableIntentsSound'
           /\ HighestAndLockAreCertified'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             PersistTimeout, CertificatesBackedByIntents,
             HonestDurableIntentsSound, HighestAndLockAreCertified
    <2>15. HonestTimeoutTransportBacked'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             PersistTimeout, HonestTimeoutTransportBacked
    <2>16. FormedTimeoutCertificatesSound'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             PersistTimeout, FormedTimeoutCertificatesSound
    <2>17. ReducerProvenanceInvariant'
      BY <2>4, <2>5, <2>11, <2>12, <2>13, <2>14, <2>15, <2>16
         DEF ReducerProvenanceInvariant
    <2>18. CurrentIntentViewsBound'
      <3>1. /\ context' = context
            /\ nodeView' = nodeView
            /\ prepareIntents' = prepareIntents
            /\ timeoutIntents' = timeoutIntents \cup {request.vote}
        BY <1>1 DEF PersistTimeout
      <3>2. \A vote \in prepareIntents':
               (vote.signer \in Honest /\ vote.context = context')
                 => vote.view <= nodeView'[vote.signer]
        BY <1>1, <3>1, Isa
         DEF StrongInductiveInvariant, LineageInvariant,
             CurrentIntentViewsBound
      <3>3. \A vote \in timeoutIntents':
               (vote.signer \in Honest /\ vote.context = context')
                 => vote.view <= nodeView'[vote.signer]
        <4>1. ASSUME NEW vote \in timeoutIntents',
                      vote.signer \in Honest,
                      vote.context = context'
               PROVE vote.view <= nodeView'[vote.signer]
          <5>1. vote \in timeoutIntents \/ vote = request.vote
            BY <3>1, <4>1
          <5>2. CASE vote \in timeoutIntents
            BY <1>1, <3>1, <4>1, <5>2
               DEF StrongInductiveInvariant, LineageInvariant,
                   CurrentIntentViewsBound
          <5>3. CASE vote = request.vote
            <6>1. /\ vote.view = request.vote.view
                  /\ vote.signer = request.vote.signer
              BY <5>3
            <6>2. /\ request.vote.view = nodeView[request.node]
                  /\ request.vote.signer = request.node
              BY <2>1
            <6>3. vote.view = nodeView[vote.signer]
              BY <6>1, <6>2
            <6>4. vote.signer \in ValidatorIds
              BY <2>1, <5>3 DEF TimeoutVoteRecordSet
            <6>5. nodeView'[vote.signer] = nodeView[vote.signer]
              BY <3>1, <6>4, Isa
            <6>6. vote.view = nodeView'[vote.signer]
              BY <6>3, <6>5, Isa
            <6>7. nodeView'[vote.signer] \in Nat
              BY <2>2, <6>4, SMT DEF TypeInvariant, Views
            <6> QED BY <6>6, <6>7, NaturalOrderReflexive
          <5> QED BY <5>1, <5>2, <5>3
        <4> QED BY <4>1
      <3> QED BY <3>2, <3>3 DEF CurrentIntentViewsBound
    <2>19. UNCHANGED
              <<context, nodeView, prepareIntents, commitIntents,
                prepareQCs, commitQCs, lockRank, lockSubject>>
      BY <1>1 DEF PersistTimeout
    <2>20. PrepareLineageSound'
      BY <1>1, <2>19, Isa
         DEF StrongInductiveInvariant, LineageInvariant,
             PrepareLineageSound, PrepareCarriesHigherSafeQc
    <2>21. /\ LocksCoverOwnCommits'
           /\ HonestCommitIntentPrepared'
           /\ CertificatePhasesCorrect'
      BY <1>1, <2>19, Isa
         DEF StrongInductiveInvariant, LineageInvariant,
             LocksCoverOwnCommits, HonestCommitIntentPrepared,
             CommitIntentsPreparedBy, CertificatePhasesCorrect
    <2>22. DurableIntentsDoNotAnticipateHeight'
      <3>1. DurableIntentsDoNotAnticipateHeight
        BY <1>1 DEF StrongInductiveInvariant, LineageInvariant
      <3>2. request.vote.context.height <= height
        BY <1>1, <2>1, SMT
           DEF StrongInductiveInvariant, Safety, TypeInvariant, Heights
      <3> QED BY <1>1, <3>1, <3>2, Isa
         DEF DurableIntentsDoNotAnticipateHeight, PersistTimeout
    <2>23. LineageInvariant'
      BY <2>18, <2>20, <2>21, <2>22 DEF LineageInvariant
    <2> QED BY <2>9, <2>10, <2>17, <2>23
       DEF StrongInductiveInvariant
  <1> QED BY <1>1

THEOREM UnchangedTimeoutIndependentProvenancePreserves ==
  ReducerProvenanceWithoutTimeoutTransport
    /\ UNCHANGED ProvenanceWithoutTimeoutTransportVars
    => ReducerProvenanceWithoutTimeoutTransport'
PROOF
  <1>1. ASSUME ReducerProvenanceWithoutTimeoutTransport,
              UNCHANGED ProvenanceWithoutTimeoutTransportVars
         PROVE ReducerProvenanceWithoutTimeoutTransport'
    <2>1. /\ HonestVoteUnique(prepareIntents)'
          /\ HonestVoteUnique(commitIntents)'
          /\ HonestTimeoutUnique(timeoutIntents)'
          /\ IntentPhasesCorrect'
          /\ PendingVoteWritesAuthorized'
          /\ PendingCertificateWritesAuthorized'
      BY <1>1, Isa
         DEF ReducerProvenanceWithoutTimeoutTransport,
             ProvenanceWithoutTimeoutTransportVars, HonestVoteUnique,
             HonestTimeoutUnique, IntentPhasesCorrect,
             PendingVoteWritesAuthorized,
             PendingCertificateWritesAuthorized,
             PrepareCarriesHigherSafeQc, NodeTimedOut,
             TCValid, HighRefValid, CurrentEpoch, CurrentVoters
    <2>2. /\ HonestVoteTransportBacked'
          /\ QcTransportBacked'
          /\ TcTransportBacked'
      BY <1>1, Isa
         DEF ReducerProvenanceWithoutTimeoutTransport,
             ProvenanceWithoutTimeoutTransportVars,
             HonestVoteTransportBacked, QcTransportBacked,
             TcTransportBacked, VoteIntentFor, TCValid, HighRefValid,
             CurrentEpoch, CurrentVoters
    <2>3. /\ CertificatesBackedByIntents'
          /\ HonestDurableIntentsSound'
      BY <1>1, Isa
         DEF ReducerProvenanceWithoutTimeoutTransport,
             ProvenanceWithoutTimeoutTransportVars,
             CertificatesBackedByIntents, HonestDurableIntentsSound
    <2>4. FormedTimeoutCertificatesSound'
      BY <1>1, Isa
         DEF ReducerProvenanceWithoutTimeoutTransport,
             ProvenanceWithoutTimeoutTransportVars,
             FormedTimeoutCertificatesSound
    <2>5. /\ DurableTimeoutsProtectCommits'
          /\ HighestAndLockAreCertified'
      BY <1>1, Isa
         DEF ReducerProvenanceWithoutTimeoutTransport,
             ProvenanceWithoutTimeoutTransportVars,
             DurableTimeoutsProtectCommits, HighestAndLockAreCertified
    <2> QED BY <2>1, <2>2, <2>3, <2>4, <2>5
       DEF ReducerProvenanceWithoutTimeoutTransport
  <1> QED BY <1>1

THEOREM CompleteTimeoutSignaturePreservesStrongInvariant ==
  \A request:
    StrongInductiveInvariant /\ CompleteTimeoutSignature(request)
      => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME NEW request,
              StrongInductiveInvariant,
              CompleteTimeoutSignature(request)
         PROVE StrongInductiveInvariant'
    <2>1. TypeInvariant'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             CompleteTimeoutSignature
    <2>2. /\ ProposalSigningRequiresIntent'
          /\ PrepareSigningRequiresIntent'
          /\ CommitSigningRequiresIntent'
          /\ TimeoutSigningRequiresIntent'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, Safety, CompleteTimeoutSignature,
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
         DEF StrongInductiveInvariant, Safety, CompleteTimeoutSignature,
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
         DEF StrongInductiveInvariant, CompleteTimeoutSignature,
             ContextIdentityBindsFrozenEpoch,
             OldContextCertificateRejected, ContextParentWasApplied,
             QcValid, CurrentEpoch
    <2>6. HonestTimeoutTransportBacked'
      BY <1>1, SMT
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             CompleteTimeoutSignature, HonestTimeoutTransportBacked,
             BroadcastTimeouts, TimeoutEnvelope
    <2>7. ReducerProvenanceWithoutTimeoutTransport'
      BY <1>1, UnchangedTimeoutIndependentProvenancePreserves
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             ReducerProvenanceWithoutTimeoutTransport,
             CompleteTimeoutSignature,
             ProvenanceWithoutTimeoutTransportVars
    <2>8. ReducerProvenanceInvariant'
      BY <2>6, <2>7
         DEF ReducerProvenanceInvariant,
             ReducerProvenanceWithoutTimeoutTransport
    <2> QED BY <1>1, <2>4, <2>5, <2>8,
                  UnchangedLineageVarsPreservesLineageInvariant
       DEF StrongInductiveInvariant, CompleteTimeoutSignature, LineageVars
  <1> QED BY <1>1

THEOREM ByzantineBroadcastTimeoutPreservesStrongInvariant ==
  \A signer, roundView, highRank, highSubject:
    StrongInductiveInvariant
      /\ ByzantineBroadcastTimeout(signer, roundView,
                                   highRank, highSubject)
      => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME NEW signer,
              NEW roundView,
              NEW highRank,
              NEW highSubject,
              StrongInductiveInvariant,
              ByzantineBroadcastTimeout(signer, roundView,
                                         highRank, highSubject)
         PROVE StrongInductiveInvariant'
    <2>1. signer \notin Honest
      BY <1>1 DEF ByzantineBroadcastTimeout, Byzantine
    <2>2. HonestTimeoutTransportBacked'
      <3>1. \A envelope \in timeoutNetwork:
               envelope.vote.signer \in Honest
                 => envelope.vote \in timeoutIntents
        BY <1>1
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               HonestTimeoutTransportBacked
      <3>2. \A envelope \in BroadcastTimeouts(
                    TimeoutVote(context, roundView, signer,
                                highRank, highSubject)):
               envelope.vote.signer \notin Honest
        BY <2>1 DEF BroadcastTimeouts, TimeoutEnvelope, TimeoutVote
      <3>3. \A envelope \in timeoutNetwork':
               envelope.vote.signer \in Honest
                 => envelope.vote \in timeoutIntents'
        BY <1>1, <3>1, <3>2, SMT
           DEF ByzantineBroadcastTimeout
      <3>4. \A received \in receivedTimeoutVotes':
               received.vote.signer \in Honest
                 => received.vote \in timeoutIntents'
        BY <1>1, Isa
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               ByzantineBroadcastTimeout, HonestTimeoutTransportBacked
      <3> QED BY <3>3, <3>4 DEF HonestTimeoutTransportBacked
    <2>3. /\ Safety'
          /\ ContextIdentityBindsFrozenEpoch'
          /\ OldContextCertificateRejected'
          /\ ContextParentWasApplied'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             OnePendingPersistencePerNode, AllPendingRequests,
             RequestsUniqueByNode, ProposalSigningRequiresIntent,
             PrepareSigningRequiresIntent, CommitSigningRequiresIntent,
             TimeoutSigningRequiresIntent, HonestPrepareUniqueness,
             HonestCommitUniqueness, HonestTimeoutUniqueness,
             LockBelowHighest, DecisionAgreement, AppliedRequiresDecision,
             ByzantineBroadcastTimeout, ContextIdentityBindsFrozenEpoch,
             OldContextCertificateRejected, ContextParentWasApplied,
             QcValid, CurrentEpoch
    <2>4. ReducerProvenanceWithoutTimeoutTransport'
      BY <1>1, UnchangedTimeoutIndependentProvenancePreserves
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             ReducerProvenanceWithoutTimeoutTransport,
             ByzantineBroadcastTimeout,
             ProvenanceWithoutTimeoutTransportVars
    <2>5. ReducerProvenanceInvariant'
      BY <2>2, <2>4
         DEF ReducerProvenanceInvariant,
             ReducerProvenanceWithoutTimeoutTransport
    <2> QED BY <1>1, <2>3, <2>5,
                  UnchangedLineageVarsPreservesLineageInvariant
       DEF StrongInductiveInvariant, ByzantineBroadcastTimeout, LineageVars
  <1> QED BY <1>1

THEOREM DeliverTimeoutPreservesStrongInvariant ==
  \A envelope:
    StrongInductiveInvariant /\ DeliverTimeout(envelope)
      => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME NEW envelope,
              StrongInductiveInvariant,
              DeliverTimeout(envelope)
         PROVE StrongInductiveInvariant'
    <2>1. HonestTimeoutTransportBacked'
      BY <1>1, SMT
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             DeliverTimeout, HonestTimeoutTransportBacked,
             TimeoutVoteAt
    <2>2. /\ Safety'
          /\ ContextIdentityBindsFrozenEpoch'
          /\ OldContextCertificateRejected'
          /\ ContextParentWasApplied'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             OnePendingPersistencePerNode, AllPendingRequests,
             RequestsUniqueByNode, ProposalSigningRequiresIntent,
             PrepareSigningRequiresIntent, CommitSigningRequiresIntent,
             TimeoutSigningRequiresIntent, HonestPrepareUniqueness,
             HonestCommitUniqueness, HonestTimeoutUniqueness,
             LockBelowHighest, DecisionAgreement, AppliedRequiresDecision,
             DeliverTimeout, ContextIdentityBindsFrozenEpoch,
             OldContextCertificateRejected, ContextParentWasApplied,
             QcValid, CurrentEpoch
    <2>3. ReducerProvenanceWithoutTimeoutTransport'
      BY <1>1, UnchangedTimeoutIndependentProvenancePreserves
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             ReducerProvenanceWithoutTimeoutTransport, DeliverTimeout,
             ProvenanceWithoutTimeoutTransportVars
    <2>4. ReducerProvenanceInvariant'
      BY <2>1, <2>3
         DEF ReducerProvenanceInvariant,
             ReducerProvenanceWithoutTimeoutTransport
    <2> QED BY <1>1, <2>2, <2>4,
                  UnchangedLineageVarsPreservesLineageInvariant
       DEF StrongInductiveInvariant, DeliverTimeout, LineageVars
  <1> QED BY <1>1

THEOREM ResumeTimeoutPreservesStrongInvariant ==
  \A node, vote:
    StrongInductiveInvariant /\ ResumeTimeout(node, vote)
      => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME NEW node,
              NEW vote,
              StrongInductiveInvariant,
              ResumeTimeout(node, vote)
         PROVE StrongInductiveInvariant'
    <2>1. TypeInvariant'
      BY <1>1, SMT
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             ResumeTimeout, TimeoutSign, TimeoutSignSet
    <2>2. TimeoutSigningRequiresIntent'
      BY <1>1, SMT
         DEF StrongInductiveInvariant, Safety, ResumeTimeout,
             TimeoutSigningRequiresIntent, TimeoutSign
    <2>3. Safety'
      BY <1>1, <2>1, <2>2, Isa
         DEF StrongInductiveInvariant, Safety, ResumeTimeout,
             OnePendingPersistencePerNode, AllPendingRequests,
             RequestsUniqueByNode, ProposalSigningRequiresIntent,
             PrepareSigningRequiresIntent, CommitSigningRequiresIntent,
             HonestPrepareUniqueness, HonestCommitUniqueness,
             HonestTimeoutUniqueness, LockBelowHighest,
             DecisionAgreement, AppliedRequiresDecision
    <2>4. /\ ContextIdentityBindsFrozenEpoch'
          /\ OldContextCertificateRejected'
          /\ ContextParentWasApplied'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, ResumeTimeout,
             ContextIdentityBindsFrozenEpoch, OldContextCertificateRejected,
             ContextParentWasApplied, QcValid, CurrentEpoch
    <2>5. ReducerProvenanceInvariant'
      BY <1>1, UnchangedProvenanceVarsPreservesReducerProvenance
         DEF StrongInductiveInvariant, ResumeTimeout, ProvenanceVars
    <2> QED BY <1>1, <2>3, <2>4, <2>5,
                  UnchangedLineageVarsPreservesLineageInvariant
       DEF StrongInductiveInvariant, ResumeTimeout, LineageVars
  <1> QED BY <1>1

THEOREM BeginObservePreparePreservesStrongInvariant ==
  \A node \in ValidatorIds:
    \A qc:
      StrongInductiveInvariant /\ BeginObservePrepare(node, qc)
        => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
              NEW qc,
              StrongInductiveInvariant,
              BeginObservePrepare(node, qc)
         PROVE StrongInductiveInvariant'
    <2> DEFINE Request == ObservePrepareWal(node, qc)
    <2>1. qc \in prepareQCs
      <3>1. qc \in prepareQCs \cup commitQCs
        BY <1>1
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               QcTransportBacked, BeginObservePrepare, QcAt
      <3>2. qc.phase = "Prepare"
        BY <1>1 DEF BeginObservePrepare
      <3>3. \A committed \in commitQCs: committed.phase = "Commit"
        BY <1>1
           DEF StrongInductiveInvariant, LineageInvariant,
               CertificatePhasesCorrect
      <3> QED BY <3>1, <3>2, <3>3
    <2>2. qc \in QcRecordSet
      BY <1>1, <2>1
         DEF StrongInductiveInvariant, Safety, TypeInvariant
    <2>3. Request \in ObservePrepareWalSet
      BY <1>1, <2>2, Isa
         DEF Request, ObservePrepareWal, ObservePrepareWalSet
    <2>4. OnePendingPersistencePerNode'
      <3>1. RequestsUniqueByNode(AllPendingRequests)
        BY <1>1
           DEF StrongInductiveInvariant, Safety,
               OnePendingPersistencePerNode
      <3>2. node \notin RequestNodeSet(AllPendingRequests)
        BY <1>1, PendingNodesAreAllRequestNodes
           DEF BeginObservePrepare, NodeIdle
      <3>3. /\ AllPendingRequests' = AllPendingRequests \cup {Request}
            /\ Request.node = node
        BY <1>1 DEF BeginObservePrepare, AllPendingRequests,
                       Request, ObservePrepareWal
      <3> QED BY <3>1, <3>2, <3>3,
                   NewRequestPreservesNodeUniqueness
         DEF OnePendingPersistencePerNode
    <2>5. TypeInvariant'
      BY <1>1, <2>3, SMT
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             BeginObservePrepare
    <2>6. PendingCertificateWritesAuthorized'
      BY <1>1, <2>1, Isa
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             PendingCertificateWritesAuthorized,
             BeginObservePrepare, Request, ObservePrepareWal,
             TCValid, HighRefValid, CurrentEpoch, CurrentVoters
    <2>7. /\ Safety'
          /\ ContextIdentityBindsFrozenEpoch'
          /\ OldContextCertificateRejected'
          /\ ContextParentWasApplied'
      BY <1>1, <2>4, <2>5, Isa
         DEF StrongInductiveInvariant, Safety, BeginObservePrepare,
             ProposalSigningRequiresIntent, PrepareSigningRequiresIntent,
             CommitSigningRequiresIntent, TimeoutSigningRequiresIntent,
             HonestPrepareUniqueness, HonestCommitUniqueness,
             HonestTimeoutUniqueness, LockBelowHighest,
             DecisionAgreement, AppliedRequiresDecision,
             ContextIdentityBindsFrozenEpoch, OldContextCertificateRejected,
             ContextParentWasApplied, QcValid, CurrentEpoch
    <2>8. ReducerProvenanceInvariant'
      <3>1. /\ HonestVoteUnique(prepareIntents)'
            /\ HonestVoteUnique(commitIntents)'
            /\ HonestTimeoutUnique(timeoutIntents)'
            /\ IntentPhasesCorrect'
            /\ PendingVoteWritesAuthorized'
            /\ HonestVoteTransportBacked'
            /\ QcTransportBacked'
            /\ HonestTimeoutTransportBacked'
            /\ TcTransportBacked'
        BY <1>1, Isa
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               BeginObservePrepare, HonestVoteUnique,
               HonestTimeoutUnique, IntentPhasesCorrect,
               PendingVoteWritesAuthorized, HonestVoteTransportBacked,
               QcTransportBacked, HonestTimeoutTransportBacked,
               TcTransportBacked, VoteIntentFor, TCValid, HighRefValid,
               CurrentEpoch, CurrentVoters,
               PrepareCarriesHigherSafeQc, NodeTimedOut
      <3>2. /\ CertificatesBackedByIntents'
            /\ HonestDurableIntentsSound'
        BY <1>1, Isa
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               BeginObservePrepare, CertificatesBackedByIntents,
               HonestDurableIntentsSound
      <3>3. FormedTimeoutCertificatesSound'
        BY <1>1, Isa
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               BeginObservePrepare, FormedTimeoutCertificatesSound
      <3>4. /\ DurableTimeoutsProtectCommits'
            /\ HighestAndLockAreCertified'
        BY <1>1, Isa
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               BeginObservePrepare, DurableTimeoutsProtectCommits,
               HighestAndLockAreCertified
      <3> QED BY <2>6, <3>1, <3>2, <3>3, <3>4
         DEF ReducerProvenanceInvariant
    <2> QED BY <1>1, <2>7, <2>8,
                  UnchangedLineageVarsPreservesLineageInvariant
       DEF StrongInductiveInvariant, BeginObservePrepare, LineageVars
  <1> QED BY <1>1

THEOREM PersistObservePreparePreservesStrongInvariant ==
  \A request:
    StrongInductiveInvariant /\ PersistObservePrepare(request)
      => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME NEW request,
              StrongInductiveInvariant,
              PersistObservePrepare(request)
         PROVE StrongInductiveInvariant'
    <2>1. /\ request.node \in ValidatorIds
          /\ request.qc \in prepareQCs
          /\ request.qc.context = context
          /\ request.qc.view > highestRank[request.node]
          /\ request.qc.view \in Views
          /\ request.qc.subject \in SubjectOrNone
      <3>1. request \in pendingObservePrepare
        BY <1>1 DEF PersistObservePrepare
      <3>2. request \in ObservePrepareWalSet
        BY <1>1, <3>1
           DEF StrongInductiveInvariant, Safety, TypeInvariant
      <3>3. /\ request.node \in ValidatorIds
            /\ request.qc \in QcRecordSet
            /\ request.qc.view \in Views
            /\ request.qc.subject \in SubjectOrNone
        BY <3>2, IsaT(120)
           DEF ObservePrepareWalSet, QcRecordSet, Subjects,
               SubjectOrNone
      <3>4. /\ request.qc \in prepareQCs
            /\ request.qc.context = context
            /\ request.qc.view > highestRank[request.node]
        BY <1>1, <3>1
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               PendingCertificateWritesAuthorized
      <3> QED BY <3>3, <3>4
    <2>2. /\ highestRank' \in [ValidatorIds -> Ranks]
          /\ highestSubject' \in [ValidatorIds -> SubjectOrNone]
      <3>1. /\ highestRank \in [ValidatorIds -> Ranks]
            /\ highestSubject \in [ValidatorIds -> SubjectOrNone]
        BY <1>1
           DEF StrongInductiveInvariant, Safety, TypeInvariant
      <3>2. /\ request.qc.view \in Ranks
            /\ request.qc.subject \in SubjectOrNone
        BY <2>1, SMT DEF Views, Ranks, NoRank
      <3>3. /\ highestRank' \in [ValidatorIds -> Ranks]
            /\ highestSubject' \in [ValidatorIds -> SubjectOrNone]
        BY <1>1, <2>1, <3>1, <3>2, Isa
           DEF PersistObservePrepare
      <3> QED BY <3>3
    <2>3. TypeInvariant'
      <3>1. /\ ModelConfiguration
            /\ height' \in Heights
            /\ context' \in ContextRecords
            /\ context'.height = height'
            /\ contextHistory' \subseteq ContextRecords
            /\ context' \in contextHistory'
            /\ nodeView' \in [ValidatorIds -> Views]
            /\ generation' \in [ValidatorIds -> Generations]
            /\ up' \subseteq ValidatorIds
            /\ gst' \in BOOLEAN
            /\ proposalIntents' \subseteq ProposalRecordSet
            /\ prepareIntents' \subseteq VoteRecordSet
            /\ commitIntents' \subseteq VoteRecordSet
            /\ timeoutIntents' \subseteq TimeoutVoteRecordSet
            /\ prepareQCs' \subseteq QcRecordSet
            /\ commitQCs' \subseteq QcRecordSet
            /\ \A tc \in formedTCs': TcWellTyped(tc)
            /\ \A entry \in receivedTCs':
                 /\ entry.node \in ValidatorIds
                 /\ TcWellTyped(entry.tc)
            /\ \A entry \in installedTCs':
                 /\ entry.node \in ValidatorIds
                 /\ TcWellTyped(entry.tc)
            /\ lockRank' \in [ValidatorIds -> Ranks]
            /\ lockSubject' \in [ValidatorIds -> SubjectOrNone]
        BY <1>1, IsaT(60)
           DEF StrongInductiveInvariant, Safety, TypeInvariant,
               PersistObservePrepare
      <3>2. /\ pendingProposal' \subseteq ProposalWalSet
            /\ pendingPrepare' \subseteq PrepareWalSet
            /\ pendingObservePrepare' \subseteq ObservePrepareWalSet
            /\ pendingLockCommit' \subseteq LockCommitWalSet
            /\ pendingTimeout' \subseteq TimeoutWalSet
            /\ \A pending \in pendingInstallTC':
                 /\ pending.node \in ValidatorIds
                 /\ pending.kind = "InstallTC"
                 /\ TcWellTyped(pending.tc)
                 /\ pending.rebroadcast \in BOOLEAN
            /\ pendingDecision' \subseteq DecisionWalSet
        BY <1>1, IsaT(60)
           DEF StrongInductiveInvariant, Safety, TypeInvariant,
               PersistObservePrepare
      <3>3. /\ signProposals' \subseteq ProposalSignSet
            /\ signVotes' \subseteq VoteSignSet
            /\ signTimeouts' \subseteq TimeoutSignSet
        BY <1>1, Isa
           DEF StrongInductiveInvariant, Safety, TypeInvariant,
               PersistObservePrepare
      <3> QED BY <2>2, <3>1, <3>2, <3>3 DEF TypeInvariant
    <2>4. OnePendingPersistencePerNode'
      BY <1>1, RemovingRequestsPreservesNodeUniqueness, Isa
         DEF StrongInductiveInvariant, Safety,
             OnePendingPersistencePerNode, AllPendingRequests,
             PersistObservePrepare
    <2>5. LockBelowHighest'
      <3>1. /\ LockBelowHighest
            /\ request.node \in ValidatorIds
            /\ request.qc.view > highestRank[request.node]
        BY <1>1, <2>1
           DEF StrongInductiveInvariant, Safety
      <3>2. ASSUME NEW node \in ValidatorIds
             PROVE lockRank'[node] <= highestRank'[node]
        <4>1. CASE node = request.node
          <5>1. /\ lockRank' = lockRank
                /\ highestRank' =
                     [highestRank EXCEPT
                        ![request.node] = request.qc.view]
            BY <1>1 DEF PersistObservePrepare
          <5>2. /\ lockRank \in [ValidatorIds -> Ranks]
                /\ lockRank' \in [ValidatorIds -> Ranks]
                /\ highestRank \in [ValidatorIds -> Ranks]
                /\ highestRank' \in [ValidatorIds -> Ranks]
            BY <1>1, <2>3
               DEF StrongInductiveInvariant, Safety, TypeInvariant
          <5>3. /\ lockRank'[node] = lockRank[node]
                /\ highestRank'[node] = request.qc.view
            BY <2>1, <3>2, <4>1, <5>1, <5>2, Isa
          <5>4. lockRank[node] <= highestRank[node]
            BY <3>1, <3>2 DEF LockBelowHighest
          <5>5. /\ lockRank[node] \in Int
                /\ highestRank[node] \in Int
                /\ request.qc.view \in Int
            BY <1>1, <2>1, <3>2, <5>2, SMT
               DEF ModelConfiguration, Views, Ranks
          <5>6. lockRank[node] < request.qc.view
            BY <3>1, <4>1, <5>4, <5>5,
               IntegerWeakStrongOrderChain
          <5>7. lockRank'[node] < highestRank'[node]
            BY <5>3, <5>6, Isa
          <5> QED BY <5>5, <5>7, IntegerStrictImpliesWeak
        <4>2. CASE node # request.node
          <5>1. /\ lockRank' = lockRank
                /\ highestRank' =
                     [highestRank EXCEPT
                        ![request.node] = request.qc.view]
            BY <1>1 DEF PersistObservePrepare
          <5>2. /\ lockRank \in [ValidatorIds -> Ranks]
                /\ lockRank' \in [ValidatorIds -> Ranks]
                /\ highestRank \in [ValidatorIds -> Ranks]
                /\ highestRank' \in [ValidatorIds -> Ranks]
            BY <1>1, <2>3
               DEF StrongInductiveInvariant, Safety, TypeInvariant
          <5>3. /\ lockRank'[node] = lockRank[node]
                /\ highestRank'[node] = highestRank[node]
            BY <3>2, <4>2, <5>1, <5>2, Isa
          <5> QED BY <3>1, <3>2, <5>3
             DEF LockBelowHighest
        <4> QED BY <4>1, <4>2
      <3> QED BY <3>2 DEF LockBelowHighest
    <2>6. PendingCertificateWritesAuthorized'
      <3>1. RequestsUniqueByNode(AllPendingRequests)
        BY <1>1
           DEF StrongInductiveInvariant, Safety,
               OnePendingPersistencePerNode
      <3>2. \A pending \in pendingObservePrepare':
               /\ pending.qc \in prepareQCs'
               /\ pending.qc.context = context'
               /\ pending.qc.view > highestRank'[pending.node]
        <4>1. ASSUME NEW pending \in pendingObservePrepare'
               PROVE /\ pending.qc \in prepareQCs'
                     /\ pending.qc.context = context'
                     /\ pending.qc.view > highestRank'[pending.node]
          <5>1. /\ pending \in pendingObservePrepare
                /\ pending # request
            BY <1>1, <4>1 DEF PersistObservePrepare
          <5>2. pending.node # request.node
            <6>1. /\ pending \in AllPendingRequests
                  /\ request \in AllPendingRequests
              BY <1>1, <5>1
                 DEF PersistObservePrepare, AllPendingRequests
            <6> QED BY <3>1, <5>1, <6>1,
                         DistinctUniqueRequestsHaveDistinctNodes
          <5>3. /\ prepareQCs' = prepareQCs
                /\ context' = context
                /\ highestRank'[pending.node] = highestRank[pending.node]
            <6>1. /\ prepareQCs' = prepareQCs
                  /\ context' = context
                  /\ highestRank' =
                       [highestRank EXCEPT
                          ![request.node] = request.qc.view]
              BY <1>1 DEF PersistObservePrepare
            <6>2. /\ pending.node \in ValidatorIds
                  /\ request.node \in ValidatorIds
                  /\ highestRank \in [ValidatorIds -> Ranks]
              BY <1>1, <2>1, <5>1
                 DEF StrongInductiveInvariant, Safety, TypeInvariant,
                     ObservePrepareWalSet
            <6> QED BY <5>2, <6>1, <6>2, Isa
          <5>4. /\ pending.qc \in prepareQCs
                /\ pending.qc.context = context
                /\ pending.qc.view > highestRank[pending.node]
            BY <1>1, <5>1
               DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
                   PendingCertificateWritesAuthorized
          <5> QED BY <5>3, <5>4
        <4> QED BY <4>1
      <3>3. /\ \A pending \in pendingInstallTC':
                     /\ pending.tc \in formedTCs'
                     /\ pending.tc.context = context'
                     /\ TCValid(pending.tc)'
                     /\ pending.tc.votes # {}
                     /\ pending.tc.view < MaxView
                     /\ pending.tc.view >= nodeView'[pending.node]
            /\ \A pending \in pendingDecision':
                     /\ pending.qc \in commitQCs'
                     /\ pending.qc.context = context'
                     /\ pending.qc.phase = "Commit"
                     /\ pending.qc.height = height'
        BY <1>1, Isa
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               PendingCertificateWritesAuthorized, PersistObservePrepare,
               TCValid, HighRefValid, CurrentEpoch, CurrentVoters
      <3> QED BY <3>2, <3>3 DEF PendingCertificateWritesAuthorized
    <2>7. HighestAndLockAreCertified'
      <3>1. /\ highestRank'[request.node] = request.qc.view
            /\ highestSubject'[request.node] = request.qc.subject
            /\ lockRank'[request.node] = lockRank[request.node]
            /\ lockSubject'[request.node] = lockSubject[request.node]
        <4>1. /\ highestRank' =
                     [highestRank EXCEPT
                        ![request.node] = request.qc.view]
              /\ highestSubject' =
                     [highestSubject EXCEPT
                        ![request.node] = request.qc.subject]
              /\ lockRank' = lockRank
              /\ lockSubject' = lockSubject
          BY <1>1 DEF PersistObservePrepare
        <4>2. /\ request.node \in ValidatorIds
              /\ highestRank \in [ValidatorIds -> Ranks]
              /\ highestSubject \in [ValidatorIds -> SubjectOrNone]
          BY <1>1, <2>1
             DEF StrongInductiveInvariant, Safety, TypeInvariant
        <4> QED BY <2>1, <4>1, <4>2, Isa
      <3>2. \A node \in ValidatorIds:
               node # request.node
                 => /\ highestRank'[node] = highestRank[node]
                    /\ highestSubject'[node] = highestSubject[node]
                    /\ lockRank'[node] = lockRank[node]
                    /\ lockSubject'[node] = lockSubject[node]
        <4>1. /\ highestRank' =
                     [highestRank EXCEPT
                        ![request.node] = request.qc.view]
              /\ highestSubject' =
                     [highestSubject EXCEPT
                        ![request.node] = request.qc.subject]
              /\ lockRank' = lockRank
              /\ lockSubject' = lockSubject
          BY <1>1 DEF PersistObservePrepare
        <4>2. ASSUME NEW node \in ValidatorIds,
                       node # request.node
               PROVE /\ highestRank'[node] = highestRank[node]
                     /\ highestSubject'[node] = highestSubject[node]
                     /\ lockRank'[node] = lockRank[node]
                     /\ lockSubject'[node] = lockSubject[node]
          <5>1. /\ request.node \in ValidatorIds
                /\ highestRank \in [ValidatorIds -> Ranks]
                /\ highestSubject \in [ValidatorIds -> SubjectOrNone]
            BY <1>1, <2>1
               DEF StrongInductiveInvariant, Safety, TypeInvariant
          <5> QED BY <4>1, <4>2, <5>1, Isa
        <4> QED BY <4>2
      <3>3. ASSUME NEW node \in ValidatorIds
             PROVE /\ (highestRank'[node] = NoRank
                          => highestSubject'[node] = NoSubject)
                   /\ (highestRank'[node] # NoRank
                          => \E qc \in prepareQCs':
                               /\ qc.context = context'
                               /\ qc.view = highestRank'[node]
                               /\ qc.subject = highestSubject'[node])
                   /\ (lockRank'[node] = NoRank
                          => lockSubject'[node] = NoSubject)
                   /\ (lockRank'[node] # NoRank
                          => \E qc \in prepareQCs':
                               /\ qc.context = context'
                               /\ qc.view = lockRank'[node]
                               /\ qc.subject = lockSubject'[node])
        <4>1. CASE node = request.node
          <5>1. request.qc.view # NoRank
            BY <2>1, ViewIsNotNoRank
          <5>2. /\ prepareQCs' = prepareQCs
                /\ context' = context
            BY <1>1 DEF PersistObservePrepare
          <5>3. /\ highestRank'[node] = request.qc.view
                /\ highestSubject'[node] = request.qc.subject
                /\ lockRank'[node] = lockRank[node]
                /\ lockSubject'[node] = lockSubject[node]
            BY <3>1, <4>1
          <5>4. /\ (highestRank'[node] = NoRank
                           => highestSubject'[node] = NoSubject)
                /\ (highestRank'[node] # NoRank
                           => \E qc \in prepareQCs':
                                /\ qc.context = context'
                                /\ qc.view = highestRank'[node]
                                /\ qc.subject = highestSubject'[node])
            <6>1. highestRank'[node] # NoRank
              BY <5>1, <5>3
            <6>2. /\ request.qc \in prepareQCs'
                  /\ request.qc.context = context'
                  /\ request.qc.view = highestRank'[node]
                  /\ request.qc.subject = highestSubject'[node]
              BY <2>1, <5>2, <5>3
            <6>3. \E qc \in prepareQCs':
                     /\ qc.context = context'
                     /\ qc.view = highestRank'[node]
                     /\ qc.subject = highestSubject'[node]
              BY <6>2
            <6> QED BY <6>1, <6>3
          <5>5. /\ (lockRank[node] = NoRank
                           => lockSubject[node] = NoSubject)
                /\ (lockRank[node] # NoRank
                           => \E qc \in prepareQCs:
                                /\ qc.context = context
                                /\ qc.view = lockRank[node]
                                /\ qc.subject = lockSubject[node])
            BY <1>1
               DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
                   HighestAndLockAreCertified
          <5> QED BY <5>2, <5>3, <5>4, <5>5
        <4>2. CASE node # request.node
          <5>1. /\ prepareQCs' = prepareQCs
                /\ context' = context
            BY <1>1 DEF PersistObservePrepare
          <5>2. /\ (highestRank[node] = NoRank
                           => highestSubject[node] = NoSubject)
                /\ (highestRank[node] # NoRank
                           => \E qc \in prepareQCs:
                                /\ qc.context = context
                                /\ qc.view = highestRank[node]
                                /\ qc.subject = highestSubject[node])
                /\ (lockRank[node] = NoRank
                           => lockSubject[node] = NoSubject)
                /\ (lockRank[node] # NoRank
                           => \E qc \in prepareQCs:
                                /\ qc.context = context
                                /\ qc.view = lockRank[node]
                                /\ qc.subject = lockSubject[node])
            BY <1>1
               DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
                   HighestAndLockAreCertified
          <5> QED BY <3>2, <4>2, <5>1, <5>2
        <4> QED BY <4>1, <4>2
      <3> QED BY <3>3 DEF HighestAndLockAreCertified
    <2>8. /\ Safety'
          /\ ContextIdentityBindsFrozenEpoch'
          /\ OldContextCertificateRejected'
          /\ ContextParentWasApplied'
      BY <1>1, <2>3, <2>4, <2>5, Isa
         DEF StrongInductiveInvariant, Safety, PersistObservePrepare,
             ProposalSigningRequiresIntent, PrepareSigningRequiresIntent,
             CommitSigningRequiresIntent, TimeoutSigningRequiresIntent,
             HonestPrepareUniqueness, HonestCommitUniqueness,
             HonestTimeoutUniqueness, DecisionAgreement,
             AppliedRequiresDecision, ContextIdentityBindsFrozenEpoch,
             OldContextCertificateRejected, ContextParentWasApplied,
             QcValid, CurrentEpoch
    <2>9. ReducerProvenanceInvariant'
      <3>1. /\ HonestVoteUnique(prepareIntents)'
            /\ HonestVoteUnique(commitIntents)'
            /\ HonestTimeoutUnique(timeoutIntents)'
            /\ IntentPhasesCorrect'
        BY <1>1, Isa
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               PersistObservePrepare, HonestVoteUnique,
               HonestTimeoutUnique, IntentPhasesCorrect
      <3>2. PendingVoteWritesAuthorized'
        BY <1>1, Isa
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               PersistObservePrepare, PendingVoteWritesAuthorized,
               PrepareCarriesHigherSafeQc, NodeTimedOut
      <3>3. /\ HonestVoteTransportBacked'
            /\ QcTransportBacked'
            /\ HonestTimeoutTransportBacked'
            /\ TcTransportBacked'
        BY <1>1, Isa
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               PersistObservePrepare, HonestVoteTransportBacked,
               QcTransportBacked, HonestTimeoutTransportBacked,
               TcTransportBacked, VoteIntentFor, TCValid, HighRefValid,
               CurrentEpoch, CurrentVoters
      <3>4. /\ CertificatesBackedByIntents'
            /\ HonestDurableIntentsSound'
            /\ FormedTimeoutCertificatesSound'
            /\ DurableTimeoutsProtectCommits'
        BY <1>1, Isa
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               PersistObservePrepare, CertificatesBackedByIntents,
               HonestDurableIntentsSound, FormedTimeoutCertificatesSound,
               DurableTimeoutsProtectCommits
      <3> QED BY <2>6, <2>7, <3>1, <3>2, <3>3, <3>4
         DEF ReducerProvenanceInvariant
    <2> QED BY <1>1, <2>8, <2>9,
                  UnchangedLineageVarsPreservesLineageInvariant
       DEF StrongInductiveInvariant, PersistObservePrepare, LineageVars
  <1> QED BY <1>1

THEOREM BeginLockCommitPreservesStrongInvariant ==
  \A node \in ValidatorIds:
    \A qc:
      StrongInductiveInvariant /\ BeginLockCommit(node, qc)
        => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
              NEW qc,
              StrongInductiveInvariant,
              BeginLockCommit(node, qc)
         PROVE StrongInductiveInvariant'
    <2> DEFINE CommitVote ==
           Vote(context, qc.view, "Commit", qc.subject, node)
    <2> DEFINE Request == LockCommitWal(node, qc, CommitVote)
    <2>1. qc \in prepareQCs
      <3>1. qc \in prepareQCs \cup commitQCs
        BY <1>1
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               QcTransportBacked, BeginLockCommit, QcAt
      <3>2. qc.phase = "Prepare"
        BY <1>1 DEF BeginLockCommit
      <3>3. \A committed \in commitQCs: committed.phase = "Commit"
        BY <1>1
           DEF StrongInductiveInvariant, LineageInvariant,
               CertificatePhasesCorrect
      <3> QED BY <3>1, <3>2, <3>3
    <2>2. CanAppendVote(commitIntents, CommitVote)
      <3>1. node \in Honest
        BY <1>1 DEF BeginLockCommit
      <3>2. ASSUME NEW prior \in commitIntents,
                    SameVoteSlot(prior, CommitVote)
             PROVE prior.subject = CommitVote.subject
        <4>1. /\ prior.signer = node
              /\ prior.context = context
              /\ prior.view = qc.view
              /\ CommitVote.subject = qc.subject
          BY <3>2 DEF CommitVote, Vote, SameVoteSlot
        <4>2. /\ lockRank[node] >= prior.view
              /\ (lockRank[node] = prior.view
                    => lockSubject[node] = prior.subject)
          BY <1>1, <3>1, <3>2, <4>1
             DEF StrongInductiveInvariant, LineageInvariant,
                 LocksCoverOwnCommits
        <4>3. /\ qc.view >= lockRank[node]
              /\ (qc.view = lockRank[node]
                    => qc.subject = lockSubject[node])
          BY <1>1 DEF BeginLockCommit
        <4>4. /\ qc.view \in Int
              /\ lockRank[node] \in Int
          <5>1. MaxView \in Nat
            BY <1>1
               DEF StrongInductiveInvariant, Safety, TypeInvariant,
                   ModelConfiguration
          <5>2. qc.view \in Views
            BY <1>1, <2>1, Isa
               DEF StrongInductiveInvariant, Safety, TypeInvariant,
                   QcRecordSet
          <5>3. lockRank[node] \in Ranks
            BY <1>1
               DEF StrongInductiveInvariant, Safety, TypeInvariant
          <5> QED BY <5>1, <5>2, <5>3, SMT
             DEF Views, Ranks, NoRank
        <4>5. qc.view = lockRank[node]
          BY <4>1, <4>2, <4>3, <4>4, SMT
        <4>6. /\ qc.subject = lockSubject[node]
              /\ prior.subject = lockSubject[node]
          BY <4>1, <4>2, <4>3, <4>5
        <4> QED BY <4>1, <4>6
      <3> QED BY <3>1, <3>2 DEF CanAppendVote, CommitVote, Vote
    <2>3. /\ Request \in LockCommitWalSet
          /\ Request.node \in Honest
          /\ Request.vote.phase = "Commit"
          /\ Request.vote.signer = Request.node
          /\ Request.vote.context = context
          /\ Request.vote.context = Request.qc.context
          /\ Request.vote.view = Request.qc.view
          /\ Request.vote.subject = Request.qc.subject
          /\ Request.qc.phase = "Prepare"
          /\ Request.qc \in prepareQCs
          /\ Request.vote.view = nodeView[Request.node]
          /\ ~NodeTimedOut(Request.node, Request.vote.view)
          /\ Request.vote.subject \in ValidSubjects
          /\ BodyHeldBy(durableBodies, Request.node,
                        Request.vote.context, Request.vote.subject)
          /\ Request.qc.view >= lockRank[Request.node]
          /\ (Request.qc.view = lockRank[Request.node]
                => Request.qc.subject = lockSubject[Request.node])
          /\ CanAppendVote(commitIntents, Request.vote)
      <3>1. /\ node \in ValidatorIds
            /\ node \in Honest
            /\ qc \in QcRecordSet
            /\ qc.context = context
            /\ qc.view \in Views
            /\ qc.subject \in ValidSubjects
            /\ qc.phase = "Prepare"
            /\ qc.view = nodeView[node]
            /\ ~NodeTimedOut(node, qc.view)
            /\ BodyHeldBy(durableBodies, node, context, qc.subject)
            /\ qc.view >= lockRank[node]
            /\ (qc.view = lockRank[node]
                  => qc.subject = lockSubject[node])
        BY <1>1, <2>1, Isa
           DEF StrongInductiveInvariant, Safety, TypeInvariant,
               ReducerProvenanceInvariant, CertificatesBackedByIntents,
               HistoricalQcValid, BeginLockCommit
      <3>2. CommitVote \in VoteRecordSet
        <4>1. /\ context \in ContextRecords
              /\ context.height \in Heights
              /\ qc.view \in Views
              /\ qc.subject \in Subjects
              /\ node \in ValidatorIds
              /\ "Commit" \in Phases
          BY <1>1, <3>1, Isa
             DEF StrongInductiveInvariant, Safety, TypeInvariant,
                 ModelConfiguration, Phases
        <4> QED BY <4>1, IsaT(120)
           DEF CommitVote, Vote, VoteRecordSet
      <3>3. /\ Request \in LockCommitWalSet
            /\ Request.node = node
            /\ Request.vote = CommitVote
            /\ Request.qc = qc
            /\ Request.vote.phase = "Commit"
            /\ Request.vote.signer = Request.node
            /\ Request.vote.context = context
            /\ Request.vote.context = Request.qc.context
            /\ Request.vote.view = Request.qc.view
            /\ Request.vote.subject = Request.qc.subject
        <4>1. Request \in LockCommitWalSet
          BY <3>1, <3>2, Isa
             DEF Request, LockCommitWal, LockCommitWalSet
        <4>2. /\ Request.node = node
              /\ Request.vote = CommitVote
              /\ Request.qc = qc
              /\ Request.vote.phase = "Commit"
              /\ Request.vote.signer = Request.node
              /\ Request.vote.context = context
              /\ Request.vote.context = Request.qc.context
              /\ Request.vote.view = Request.qc.view
              /\ Request.vote.subject = Request.qc.subject
          BY <3>1, Isa
             DEF Request, CommitVote, LockCommitWal, Vote
        <4> QED BY <4>1, <4>2
      <3> QED BY <1>1, <2>1, <2>2, <3>1, <3>3
         DEF BeginLockCommit, Request, LockCommitWal
    <2>4. OnePendingPersistencePerNode'
      <3>1. RequestsUniqueByNode(AllPendingRequests)
        BY <1>1
           DEF StrongInductiveInvariant, Safety,
               OnePendingPersistencePerNode
      <3>2. node \notin RequestNodeSet(AllPendingRequests)
        BY <1>1, PendingNodesAreAllRequestNodes
           DEF BeginLockCommit, NodeIdle
      <3>3. /\ AllPendingRequests' = AllPendingRequests \cup {Request}
            /\ Request.node = node
        BY <1>1 DEF BeginLockCommit, AllPendingRequests,
                       Request, LockCommitWal
      <3> QED BY <3>1, <3>2, <3>3,
                   NewRequestPreservesNodeUniqueness
         DEF OnePendingPersistencePerNode
    <2>5. TypeInvariant'
      BY <1>1, <2>3, Isa
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             BeginLockCommit, Request
    <2>6. PendingVoteWritesAuthorized'
      <3>1. PendingVoteWritesAuthorized
        BY <1>1
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant
      <3>2. pendingLockCommit' = pendingLockCommit \cup {Request}
        BY <1>1 DEF BeginLockCommit, Request
      <3>3. \A pending \in pendingLockCommit':
               /\ pending.node \in Honest
               /\ pending.vote.phase = "Commit"
               /\ pending.vote.signer = pending.node
               /\ pending.vote.context = context'
               /\ pending.vote.context = pending.qc.context
               /\ pending.vote.view = pending.qc.view
               /\ pending.vote.subject = pending.qc.subject
               /\ pending.qc.phase = "Prepare"
               /\ pending.qc \in prepareQCs'
               /\ pending.vote.view = nodeView'[pending.node]
               /\ ~NodeTimedOut(pending.node, pending.vote.view)'
               /\ pending.vote.subject \in ValidSubjects
               /\ BodyHeldBy(durableBodies', pending.node,
                             pending.vote.context, pending.vote.subject)
               /\ pending.qc.view >= lockRank'[pending.node]
               /\ (pending.qc.view = lockRank'[pending.node]
                     => pending.qc.subject = lockSubject'[pending.node])
               /\ CanAppendVote(commitIntents', pending.vote)
        <4>1. ASSUME NEW pending \in pendingLockCommit'
               PROVE /\ pending.node \in Honest
                     /\ pending.vote.phase = "Commit"
                     /\ pending.vote.signer = pending.node
                     /\ pending.vote.context = context'
                     /\ pending.vote.context = pending.qc.context
                     /\ pending.vote.view = pending.qc.view
                     /\ pending.vote.subject = pending.qc.subject
                     /\ pending.qc.phase = "Prepare"
                     /\ pending.qc \in prepareQCs'
                     /\ pending.vote.view = nodeView'[pending.node]
                     /\ ~NodeTimedOut(pending.node, pending.vote.view)'
                     /\ pending.vote.subject \in ValidSubjects
                     /\ BodyHeldBy(durableBodies', pending.node,
                                   pending.vote.context,
                                   pending.vote.subject)
                     /\ pending.qc.view >= lockRank'[pending.node]
                     /\ (pending.qc.view = lockRank'[pending.node]
                           => pending.qc.subject = lockSubject'[pending.node])
                     /\ CanAppendVote(commitIntents', pending.vote)
          <5>1. pending \in pendingLockCommit \/ pending = Request
            BY <3>2, <4>1
          <5>2. CASE pending \in pendingLockCommit
            <6>1. /\ pending.node \in Honest
                  /\ pending.vote.phase = "Commit"
                  /\ pending.vote.signer = pending.node
                  /\ pending.vote.context = context
                  /\ pending.vote.context = pending.qc.context
                  /\ pending.vote.view = pending.qc.view
                  /\ pending.vote.subject = pending.qc.subject
                  /\ pending.qc.phase = "Prepare"
                  /\ pending.qc \in prepareQCs
                  /\ pending.vote.view = nodeView[pending.node]
                  /\ ~NodeTimedOut(pending.node, pending.vote.view)
                  /\ pending.vote.subject \in ValidSubjects
                  /\ BodyHeldBy(durableBodies, pending.node,
                                pending.vote.context,
                                pending.vote.subject)
                  /\ pending.qc.view >= lockRank[pending.node]
                  /\ (pending.qc.view = lockRank[pending.node]
                        => pending.qc.subject = lockSubject[pending.node])
                  /\ CanAppendVote(commitIntents, pending.vote)
              BY <3>1, <5>2 DEF PendingVoteWritesAuthorized
            <6>2. /\ context' = context
                  /\ nodeView' = nodeView
                  /\ durableBodies' = durableBodies
                  /\ lockRank' = lockRank
                  /\ lockSubject' = lockSubject
                  /\ commitIntents' = commitIntents
              BY <1>1 DEF BeginLockCommit
            <6> QED BY <6>1, <6>2
          <5>3. CASE pending = Request
            BY <1>1, <2>3, <5>3, Isa DEF BeginLockCommit
          <5> QED BY <5>1, <5>2, <5>3
        <4> QED BY <4>1
      <3>4. /\ \A pending \in pendingPrepare':
                     /\ pending.node \in Honest
                     /\ pending.vote.phase = "Prepare"
                     /\ pending.vote.signer = pending.node
                     /\ pending.vote.context = context'
                     /\ pending.vote.view = nodeView'[pending.node]
                     /\ pending.vote.subject \in ValidSubjects
                     /\ BodyHeldBy(durableBodies', pending.node,
                                   pending.vote.context,
                                   pending.vote.subject)
                     /\ CanAppendVote(prepareIntents', pending.vote)
                     /\ PrepareCarriesHigherSafeQc(pending.vote)'
            /\ \A pending \in pendingTimeout':
                     /\ pending.node \in Honest
                     /\ pending.vote.signer = pending.node
                     /\ pending.vote.context = context'
                     /\ pending.vote.view = nodeView'[pending.node]
                     /\ CanAppendTimeout(timeoutIntents', pending.vote)
                     /\ TimeoutVoteProtectsCommitSet(
                          pending.vote, commitIntents')
        BY <1>1, <3>1, Isa
           DEF BeginLockCommit, PendingVoteWritesAuthorized,
               PrepareCarriesHigherSafeQc, NodeTimedOut
      <3> QED BY <3>3, <3>4
         DEF PendingVoteWritesAuthorized, NodeTimedOut
    <2>7. /\ Safety'
          /\ ContextIdentityBindsFrozenEpoch'
          /\ OldContextCertificateRejected'
          /\ ContextParentWasApplied'
      BY <1>1, <2>4, <2>5, Isa
         DEF StrongInductiveInvariant, Safety, BeginLockCommit,
             ProposalSigningRequiresIntent, PrepareSigningRequiresIntent,
             CommitSigningRequiresIntent, TimeoutSigningRequiresIntent,
             HonestPrepareUniqueness, HonestCommitUniqueness,
             HonestTimeoutUniqueness, LockBelowHighest,
             DecisionAgreement, AppliedRequiresDecision,
             ContextIdentityBindsFrozenEpoch, OldContextCertificateRejected,
             ContextParentWasApplied, QcValid, CurrentEpoch
    <2>8. ReducerProvenanceInvariant'
      <3>1. /\ HonestVoteUnique(prepareIntents)'
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
        <4>1. /\ HonestVoteUnique(prepareIntents)'
              /\ HonestVoteUnique(commitIntents)'
              /\ HonestTimeoutUnique(timeoutIntents)'
              /\ IntentPhasesCorrect'
          BY <1>1, Isa
             DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
                 BeginLockCommit, HonestVoteUnique,
                 HonestTimeoutUnique, IntentPhasesCorrect
        <4>2. PendingCertificateWritesAuthorized'
          BY <1>1, Isa
             DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
                 BeginLockCommit, PendingCertificateWritesAuthorized,
                 TCValid, HighRefValid, CurrentEpoch, CurrentVoters
        <4>3. /\ HonestVoteTransportBacked'
              /\ QcTransportBacked'
              /\ HonestTimeoutTransportBacked'
              /\ TcTransportBacked'
          BY <1>1, Isa
             DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
                 BeginLockCommit, HonestVoteTransportBacked,
                 QcTransportBacked, HonestTimeoutTransportBacked,
                 TcTransportBacked, VoteIntentFor, TCValid, HighRefValid,
                 CurrentEpoch, CurrentVoters
        <4>4. /\ CertificatesBackedByIntents'
              /\ HonestDurableIntentsSound'
              /\ FormedTimeoutCertificatesSound'
              /\ DurableTimeoutsProtectCommits'
              /\ HighestAndLockAreCertified'
          BY <1>1, Isa
             DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
                 BeginLockCommit, CertificatesBackedByIntents,
                 HonestDurableIntentsSound, FormedTimeoutCertificatesSound,
                 DurableTimeoutsProtectCommits, HighestAndLockAreCertified
        <4> QED BY <4>1, <4>2, <4>3, <4>4
      <3> QED BY <2>6, <3>1 DEF ReducerProvenanceInvariant
    <2> QED BY <1>1, <2>7, <2>8,
                  UnchangedLineageVarsPreservesLineageInvariant
       DEF StrongInductiveInvariant, BeginLockCommit, LineageVars
  <1> QED BY <1>1

(***************************************************************************
The remaining certificate and WAL acknowledgements are proved here instead
of being hidden behind the top-level Next disjunction.  In particular, the
pending-request invariant retains every admission guard needed after an
arbitrary number of unrelated asynchronous transitions.
***************************************************************************)

THEOREM PersistLockCommitPreservesStrongInvariant ==
  \A request:
    StrongInductiveInvariant /\ PersistLockCommit(request)
      => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME NEW request,
              StrongInductiveInvariant,
              PersistLockCommit(request)
         PROVE StrongInductiveInvariant'
    <2>1. /\ request \in pendingLockCommit
          /\ request.node \in Honest
          /\ request.qc \in prepareQCs
          /\ request.qc.phase = "Prepare"
          /\ request.vote.phase = "Commit"
          /\ request.vote.signer = request.node
          /\ request.vote.context = context
          /\ request.vote.context = request.qc.context
          /\ request.vote.view = request.qc.view
          /\ request.vote.subject = request.qc.subject
          /\ request.vote.view = nodeView[request.node]
          /\ ~NodeTimedOut(request.node, request.vote.view)
          /\ request.vote.subject \in ValidSubjects
          /\ BodyHeldBy(durableBodies, request.node,
                        request.vote.context, request.vote.subject)
          /\ request.qc.view >= lockRank[request.node]
          /\ (request.qc.view = lockRank[request.node]
                => request.qc.subject = lockSubject[request.node])
          /\ CanAppendVote(commitIntents, request.vote)
      BY <1>1
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             PendingVoteWritesAuthorized, PersistLockCommit
    <2>2. HonestVoteUnique(commitIntents')
      BY <1>1, <2>1, DurableVoteAppendPreservesUniqueness
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             PersistLockCommit
    <2>3. OnePendingPersistencePerNode'
      BY <1>1, RemovingRequestsPreservesNodeUniqueness, Isa
         DEF StrongInductiveInvariant, Safety,
             OnePendingPersistencePerNode, AllPendingRequests,
             PersistLockCommit
    <2>4. TypeInvariant'
      <3>1. /\ request.node \in ValidatorIds
            /\ request.vote \in VoteRecordSet
            /\ request.qc.view \in Views
            /\ request.qc.subject \in Subjects
        <4>1. TypeInvariant
          BY <1>1 DEF StrongInductiveInvariant, Safety
        <4>2. /\ QuorumConfiguration
              /\ ValidSubjects \subseteq Subjects
          BY <4>1 DEF TypeInvariant, ModelConfiguration
        <4>3. request.node \in ValidatorIds
          BY <2>1, <4>2 DEF QuorumConfiguration
        <4>4. request.vote \in VoteRecordSet
          <5>1. pendingLockCommit \subseteq LockCommitWalSet
            BY <4>1 DEF TypeInvariant
          <5>2. request \in pendingLockCommit
            BY <1>1 DEF PersistLockCommit
          <5>3. request \in LockCommitWalSet
            BY <5>1, <5>2
          <5> QED BY <5>3 DEF LockCommitWalSet
        <4>5. request.qc \in QcRecordSet
          BY <2>1, <4>1 DEF TypeInvariant
        <4>6. request.qc.view \in Views
          BY <4>5 DEF QcRecordSet
        <4>7. request.qc.subject \in Subjects
          BY <2>1, <4>2 DEF QcRecordSet
        <4> QED BY <4>3, <4>4, <4>6, <4>7
      <3>2. /\ ModelConfiguration'
            /\ height' \in Heights
            /\ context' \in ContextRecords
            /\ context'.height = height'
            /\ contextHistory' \subseteq ContextRecords
            /\ context' \in contextHistory'
            /\ nodeView' \in [ValidatorIds -> Views]
            /\ generation' \in [ValidatorIds -> Generations]
            /\ up' \subseteq ValidatorIds
            /\ gst' \in BOOLEAN
            /\ proposalIntents' \subseteq ProposalRecordSet
            /\ prepareIntents' \subseteq VoteRecordSet
            /\ timeoutIntents' \subseteq TimeoutVoteRecordSet
            /\ prepareQCs' \subseteq QcRecordSet
            /\ commitQCs' \subseteq QcRecordSet
            /\ \A tc \in formedTCs': TcWellTyped(tc)
            /\ \A entry \in receivedTCs':
                 /\ entry.node \in ValidatorIds
                 /\ TcWellTyped(entry.tc)
            /\ \A entry \in installedTCs':
                 /\ entry.node \in ValidatorIds
                 /\ TcWellTyped(entry.tc)
            /\ pendingProposal' \subseteq ProposalWalSet
            /\ pendingPrepare' \subseteq PrepareWalSet
            /\ pendingObservePrepare' \subseteq ObservePrepareWalSet
            /\ pendingTimeout' \subseteq TimeoutWalSet
            /\ \A pending \in pendingInstallTC':
                 /\ pending.node \in ValidatorIds
                 /\ pending.kind = "InstallTC"
                 /\ TcWellTyped(pending.tc)
                 /\ pending.rebroadcast \in BOOLEAN
            /\ pendingDecision' \subseteq DecisionWalSet
            /\ signProposals' \subseteq ProposalSignSet
            /\ signTimeouts' \subseteq TimeoutSignSet
        BY <1>1, Isa
           DEF StrongInductiveInvariant, Safety, TypeInvariant,
               PersistLockCommit
      <3>3. commitIntents' \subseteq VoteRecordSet
        BY <1>1, <3>1, SMT
           DEF StrongInductiveInvariant, Safety, TypeInvariant,
               PersistLockCommit
      <3>4. /\ lockRank' \in [ValidatorIds -> Ranks]
            /\ lockSubject' \in [ValidatorIds -> SubjectOrNone]
            /\ highestRank' \in [ValidatorIds -> Ranks]
            /\ highestSubject' \in [ValidatorIds -> SubjectOrNone]
        <4>1. /\ lockRank \in [ValidatorIds -> Ranks]
              /\ lockSubject \in [ValidatorIds -> SubjectOrNone]
              /\ highestRank \in [ValidatorIds -> Ranks]
              /\ highestSubject \in [ValidatorIds -> SubjectOrNone]
          BY <1>1
             DEF StrongInductiveInvariant, Safety, TypeInvariant
        <4>2. /\ request.qc.view \in Ranks
              /\ request.qc.subject \in SubjectOrNone
              /\ highestRank[request.node] \in Ranks
              /\ highestSubject[request.node] \in SubjectOrNone
          <5>1. request.qc.view \in Ranks
            BY <3>1, ViewsAreRanks
          <5>2. request.qc.subject \in SubjectOrNone
            BY <3>1, SubjectsAreSubjectOrNone
          <5>3. highestRank[request.node] \in Ranks
            BY <3>1, <4>1, FunctionValueHasCodomain
          <5>4. highestSubject[request.node] \in SubjectOrNone
            BY <3>1, <4>1, FunctionValueHasCodomain
          <5> QED BY <5>1, <5>2, <5>3, <5>4
        <4> DEFINE NextHighestRank ==
             IF request.qc.view > highestRank[request.node]
             THEN request.qc.view ELSE highestRank[request.node]
        <4> DEFINE NextHighestSubject ==
             IF request.qc.view > highestRank[request.node]
             THEN request.qc.subject ELSE highestSubject[request.node]
        <4>3. /\ NextHighestRank \in Ranks
              /\ NextHighestSubject \in SubjectOrNone
          BY <4>2 DEF NextHighestRank, NextHighestSubject
        <4>4. /\ lockRank'
                    = [lockRank EXCEPT
                         ![request.node] = request.qc.view]
              /\ lockSubject'
                    = [lockSubject EXCEPT
                         ![request.node] = request.qc.subject]
              /\ highestRank'
                    = [highestRank EXCEPT
                         ![request.node] = NextHighestRank]
              /\ highestSubject'
                    = [highestSubject EXCEPT
                         ![request.node] = NextHighestSubject]
          BY <1>1 DEF PersistLockCommit,
                         NextHighestRank, NextHighestSubject
        <4>5. lockRank' \in [ValidatorIds -> Ranks]
          BY <3>1, <4>1, <4>2, <4>4,
             FunctionalUpdatePreservesType
        <4>6. lockSubject' \in [ValidatorIds -> SubjectOrNone]
          BY <3>1, <4>1, <4>2, <4>4,
             FunctionalUpdatePreservesType
        <4>7. highestRank' \in [ValidatorIds -> Ranks]
          BY <3>1, <4>1, <4>3, <4>4,
             FunctionalUpdatePreservesType
        <4>8. highestSubject' \in [ValidatorIds -> SubjectOrNone]
          BY <3>1, <4>1, <4>3, <4>4,
             FunctionalUpdatePreservesType
        <4> QED BY <4>5, <4>6, <4>7, <4>8
      <3>5. pendingLockCommit' \subseteq LockCommitWalSet
        BY <1>1, Isa
           DEF StrongInductiveInvariant, Safety, TypeInvariant,
               PersistLockCommit
      <3>6. signVotes' \subseteq VoteSignSet
        <4>1. signVotes \subseteq VoteSignSet
          BY <1>1
             DEF StrongInductiveInvariant, Safety, TypeInvariant
        <4>2. VoteSign(request.node, request.vote) \in VoteSignSet
          BY <3>1 DEF VoteSign, VoteSignSet
        <4>3. signVotes'
              = signVotes \cup {VoteSign(request.node, request.vote)}
          BY <1>1 DEF PersistLockCommit
        <4> QED BY <4>1, <4>2, <4>3
      <3> QED BY <3>2, <3>3, <3>4, <3>5, <3>6
         DEF TypeInvariant
    <2>5. LockBelowHighest'
      BY <1>1, <2>1, SMT
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             LockBelowHighest, PersistLockCommit, Ranks, Views
    <2>6. Safety'
      <3>1. IntentPhasesCorrect'
        BY <1>1, <2>1, SMT
           DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
               IntentPhasesCorrect, PersistLockCommit
      <3>2. HonestCommitUniqueness'
        BY <2>2, <3>1, PhasedVoteUniquenessImpliesSlotUniqueness
           DEF HonestCommitUniqueness, IntentPhasesCorrect
      <3>3. PrepareSigningRequiresIntent'
        BY <1>1, <2>1, SMT
           DEF StrongInductiveInvariant, Safety, PersistLockCommit,
               PrepareSigningRequiresIntent, VoteSign
      <3>4. CommitSigningRequiresIntent'
        BY <1>1, <2>1, SMT
           DEF StrongInductiveInvariant, Safety, PersistLockCommit,
               CommitSigningRequiresIntent, VoteSign
      <3>5. /\ ProposalSigningRequiresIntent'
            /\ TimeoutSigningRequiresIntent'
            /\ HonestPrepareUniqueness'
            /\ HonestTimeoutUniqueness'
            /\ DecisionAgreement'
            /\ AppliedRequiresDecision'
        BY <1>1, Isa
           DEF StrongInductiveInvariant, Safety, PersistLockCommit,
               ProposalSigningRequiresIntent,
               TimeoutSigningRequiresIntent,
               HonestPrepareUniqueness, HonestTimeoutUniqueness,
               DecisionAgreement, AppliedRequiresDecision
      <3> QED BY <2>3, <2>4, <2>5, <3>2, <3>3, <3>4, <3>5
         DEF Safety
    <2>7. ReducerProvenanceInvariant'
      BY <1>1, <2>1, <2>2, IsaT(240)
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             PersistLockCommit, HonestVoteUnique, HonestTimeoutUnique,
             IntentPhasesCorrect, PendingVoteWritesAuthorized,
             PendingCertificateWritesAuthorized,
             HonestVoteTransportBacked, QcTransportBacked,
             HonestTimeoutTransportBacked, TcTransportBacked,
             CertificatesBackedByIntents, HonestDurableIntentsSound,
             HonestIntentSound, FormedTimeoutCertificatesSound,
             DurableTimeoutsProtectCommits, HighestAndLockAreCertified,
             TimeoutIntentProtectsCommits, TimeoutVoteProtectsCommitSet,
             CurrentIntentViewsBound, NodeTimedOut, VoteIntentFor,
             PrepareCarriesHigherSafeQc, RequestsUniqueByNode,
             AllPendingRequests
    <2>8. LineageInvariant'
      BY <1>1, <2>1, IsaT(240)
         DEF StrongInductiveInvariant, LineageInvariant,
             PersistLockCommit, PrepareLineageSound,
             PrepareCarriesHigherSafeQc, LocksCoverOwnCommits,
             CurrentIntentViewsBound, HonestCommitIntentPrepared,
             CommitIntentsPreparedBy, CertificatePhasesCorrect,
             DurableIntentsDoNotAnticipateHeight,
             RequestsUniqueByNode, AllPendingRequests
    <2>9. /\ ContextIdentityBindsFrozenEpoch'
          /\ OldContextCertificateRejected'
          /\ ContextParentWasApplied'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, PersistLockCommit,
             ContextIdentityBindsFrozenEpoch,
             OldContextCertificateRejected, ContextParentWasApplied,
             QcValid, CurrentEpoch
    <2> QED BY <2>6, <2>7, <2>8, <2>9
       DEF StrongInductiveInvariant
  <1> QED BY <1>1

THEOREM FormCommitQCPreservesStrongInvariant ==
  \A node, roundView, subject:
    StrongInductiveInvariant /\ FormCommitQC(node, roundView, subject)
      => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME NEW node, NEW roundView, NEW subject,
              StrongInductiveInvariant,
              FormCommitQC(node, roundView, subject)
         PROVE StrongInductiveInvariant'
    <2> DEFINE Signers ==
           VoteSignersAt(node, roundView, "Commit", subject)
    <2> DEFINE Certificate ==
           QC(context, roundView, "Commit", subject, Signers)
    <2> DEFINE Request == DecisionWal(node, Certificate, TRUE)
    <2>1. /\ Certificate \in QcRecordSet
          /\ QcValid(Certificate)
          /\ CertificateHonestIntentBacked(Certificate, commitIntents)
          /\ Certificate.phase = "Commit"
          /\ commitQCs' = commitQCs \cup {Certificate}
          /\ pendingDecision' = pendingDecision \cup {Request}
      BY <1>1
         DEF FormCommitQC, Certificate, Signers, Request, QC
    <2>2. OnePendingPersistencePerNode'
      BY <1>1, PendingNodesAreAllRequestNodes,
         NewRequestPreservesNodeUniqueness, Isa
         DEF StrongInductiveInvariant, Safety,
             OnePendingPersistencePerNode, FormCommitQC,
             AllPendingRequests, NodeIdle, Request, DecisionWal
    <2>3. /\ TypeInvariant'
          /\ PendingCertificateWritesAuthorized'
          /\ CertificatesBackedByIntents'
      BY <1>1, <2>1, IsaT(180)
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             ReducerProvenanceInvariant,
             PendingCertificateWritesAuthorized,
             CertificatesBackedByIntents, HistoricalQcValid,
             CertificateBackedBy, FormCommitQC,
             Certificate, Signers, Request, DecisionWal, QC,
             QcValid, CurrentEpoch
    <2>4. /\ Safety'
          /\ ReducerProvenanceInvariant'
          /\ LineageInvariant'
      BY <1>1, <2>1, <2>2, <2>3, IsaT(180)
         DEF StrongInductiveInvariant, Safety,
             ReducerProvenanceInvariant, LineageInvariant,
             FormCommitQC, HonestVoteUnique, HonestTimeoutUnique,
             IntentPhasesCorrect, PendingVoteWritesAuthorized,
             PendingCertificateWritesAuthorized,
             HonestVoteTransportBacked, QcTransportBacked,
             HonestTimeoutTransportBacked, TcTransportBacked,
             CertificatesBackedByIntents, HonestDurableIntentsSound,
             FormedTimeoutCertificatesSound,
             DurableTimeoutsProtectCommits, HighestAndLockAreCertified,
             CertificatePhasesCorrect, DurableIntentsDoNotAnticipateHeight,
             DecisionAgreement, AllPendingRequests, Request, Certificate
    <2>5. /\ ContextIdentityBindsFrozenEpoch'
          /\ OldContextCertificateRejected'
          /\ ContextParentWasApplied'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, FormCommitQC,
             ContextIdentityBindsFrozenEpoch,
             OldContextCertificateRejected, ContextParentWasApplied,
             QcValid, CurrentEpoch
    <2> QED BY <2>4, <2>5 DEF StrongInductiveInvariant
  <1> QED BY <1>1

THEOREM BeginDecisionPreservesStrongInvariant ==
  \A node, qc:
    StrongInductiveInvariant /\ BeginDecision(node, qc)
      => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME NEW node, NEW qc,
              StrongInductiveInvariant,
              BeginDecision(node, qc)
         PROVE StrongInductiveInvariant'
    <2> DEFINE Request == DecisionWal(node, qc, FALSE)
    <2>1. /\ qc \in commitQCs
          /\ qc.phase = "Commit"
          /\ qc.context = context
          /\ Request \in DecisionWalSet
      BY <1>1, Isa
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             ReducerProvenanceInvariant, QcTransportBacked,
             LineageInvariant, CertificatePhasesCorrect,
             BeginDecision, QcAt, Request, DecisionWal,
             DecisionWalSet
    <2>2. OnePendingPersistencePerNode'
      BY <1>1, PendingNodesAreAllRequestNodes,
         NewRequestPreservesNodeUniqueness, Isa
         DEF StrongInductiveInvariant, Safety,
             OnePendingPersistencePerNode, BeginDecision,
             AllPendingRequests, NodeIdle, Request, DecisionWal
    <2>3. /\ TypeInvariant'
          /\ PendingCertificateWritesAuthorized'
      BY <1>1, <2>1, Isa
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             ReducerProvenanceInvariant,
             PendingCertificateWritesAuthorized,
             BeginDecision, Request, HistoricalQcValid,
             CertificatesBackedByIntents
    <2>4. /\ Safety'
          /\ ReducerProvenanceInvariant'
          /\ LineageInvariant'
          /\ ContextIdentityBindsFrozenEpoch'
          /\ OldContextCertificateRejected'
          /\ ContextParentWasApplied'
      BY <1>1, <2>1, <2>2, <2>3, IsaT(120)
         DEF StrongInductiveInvariant, Safety,
             ReducerProvenanceInvariant, LineageInvariant,
             BeginDecision, PendingCertificateWritesAuthorized,
             ContextIdentityBindsFrozenEpoch,
             OldContextCertificateRejected, ContextParentWasApplied,
             DecisionAgreement, AllPendingRequests, QcValid, CurrentEpoch
    <2> QED BY <2>4 DEF StrongInductiveInvariant
  <1> QED BY <1>1

THEOREM PersistDecisionPreservesStrongInvariant ==
  \A request:
    StrongInductiveInvariant /\ PersistDecision(request)
      => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME NEW request,
              StrongInductiveInvariant,
              PersistDecision(request)
         PROVE StrongInductiveInvariant'
    <2> DEFINE Decision == [node |-> request.node, qc |-> request.qc]
    <2>1. /\ request \in pendingDecision
          /\ request.qc \in commitQCs
          /\ request.qc.phase = "Commit"
          /\ decisions' = decisions \cup {Decision}
      BY <1>1
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             PendingCertificateWritesAuthorized,
             PersistDecision, Decision
    <2>2. \A left, right \in commitQCs:
             left.context = right.context
               => left.subject = right.subject
      BY <1>1, CommitCertificateAgreement
         DEF StrongInductiveInvariant, Safety,
             ReducerProvenanceInvariant, LineageInvariant
    <2>3. DecisionAgreement'
      BY <1>1, <2>1, <2>2, Isa
         DEF StrongInductiveInvariant, Safety, DecisionAgreement,
             PersistDecision, Decision
    <2>4. OnePendingPersistencePerNode'
      BY <1>1, RemovingRequestsPreservesNodeUniqueness, Isa
         DEF StrongInductiveInvariant, Safety,
             OnePendingPersistencePerNode, AllPendingRequests,
             PersistDecision
    <2>5. /\ Safety'
          /\ ReducerProvenanceInvariant'
          /\ LineageInvariant'
          /\ ContextIdentityBindsFrozenEpoch'
          /\ OldContextCertificateRejected'
          /\ ContextParentWasApplied'
      BY <1>1, <2>1, <2>3, <2>4, IsaT(180)
         DEF StrongInductiveInvariant, Safety,
             ReducerProvenanceInvariant, LineageInvariant,
             PersistDecision, PendingCertificateWritesAuthorized,
             QcTransportBacked, TcTransportBacked,
             ContextIdentityBindsFrozenEpoch,
             OldContextCertificateRejected, ContextParentWasApplied,
             AppliedRequiresDecision, AllPendingRequests,
             QcValid, CurrentEpoch, Decision
    <2> QED BY <2>5 DEF StrongInductiveInvariant
  <1> QED BY <1>1

THEOREM ValidTimeoutCertificateSelectsReportedMaximum ==
  \A tc:
    TCValid(tc) => TCMaximumProtectsReports(tc)
BY Isa
   DEF TCValid, TCMaximumProtectsReports, TcHighRank, TcHighSubject,
       HighestTimeoutVote, TimeoutHighsConflictFree, HighRefValid

THEOREM FormTCPreservesStrongInvariant ==
  \A node, roundView:
    StrongInductiveInvariant /\ FormTC(node, roundView)
      => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME NEW node, NEW roundView,
              StrongInductiveInvariant,
              FormTC(node, roundView)
         PROVE StrongInductiveInvariant'
    <2> DEFINE Votes == TimeoutVotesAt(node, roundView)
    <2> DEFINE Certificate == TC(context, roundView, Votes)
    <2> DEFINE Request == InstallTcWal(node, Certificate, TRUE)
    <2>1. /\ TCValid(Certificate)
          /\ Certificate.votes # {}
          /\ Certificate.view < MaxView
          /\ Certificate.view >= nodeView[node]
          /\ formedTCs' = formedTCs \cup {Certificate}
          /\ pendingInstallTC' = pendingInstallTC \cup {Request}
      BY <1>1
         DEF FormTC, Certificate, Votes, Request, TC
    <2>2. OnePendingPersistencePerNode'
      BY <1>1, PendingNodesAreAllRequestNodes,
         NewRequestPreservesNodeUniqueness, Isa
         DEF StrongInductiveInvariant, Safety,
             OnePendingPersistencePerNode, FormTC,
             AllPendingRequests, NodeIdle, Request, InstallTcWal
    <2>3. /\ TcWellTyped(Certificate)
          /\ Request.node \in ValidatorIds
          /\ Request.kind = "InstallTC"
          /\ TcWellTyped(Request.tc)
          /\ Request.rebroadcast \in BOOLEAN
      BY <1>1, <2>1, IsaT(120)
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             TCValid, TcWellTyped, TimeoutVotesAt,
             Certificate, Votes, Request, TC, InstallTcWal,
             ContextRecords, Heights, Views, Ranks, HighRefValid,
             CurrentVoters, ModelConfiguration
    <2>4. TypeInvariant'
      BY <1>1, <2>1, <2>3, Isa
         DEF StrongInductiveInvariant, Safety, TypeInvariant, FormTC,
             Certificate, Request
    <2>5. OnePendingPersistencePerNode
      BY <1>1 DEF StrongInductiveInvariant, Safety
    <2>6. FormedTimeoutCertificatesSound'
      BY <1>1, <2>1, <2>5,
         ValidTimeoutCertificateSelectsReportedMaximum, IsaT(240)
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             FormedTimeoutCertificatesSound, FormTC,
             Certificate, Votes, Request, TC, TCValid,
             TimeoutVotesAt, TimeoutVoteAt,
             HonestTimeoutTransportBacked,
             TimeoutVotesBindCertificate, HistoricalQcValid,
             RequestsUniqueByNode, AllPendingRequests,
             CurrentEpoch, CurrentVoters, HighRefValid
    <2>7. /\ PendingCertificateWritesAuthorized'
          /\ TcTransportBacked'
      BY <1>1, <2>1, <2>2, IsaT(120)
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             PendingCertificateWritesAuthorized, TcTransportBacked,
             FormTC, Certificate, Request
    <2>8. /\ Safety'
          /\ ReducerProvenanceInvariant'
          /\ LineageInvariant'
          /\ ContextIdentityBindsFrozenEpoch'
          /\ OldContextCertificateRejected'
          /\ ContextParentWasApplied'
      BY <1>1, <2>1, <2>2, <2>4, <2>6, <2>7, IsaT(180)
         DEF StrongInductiveInvariant, Safety,
             ReducerProvenanceInvariant, LineageInvariant,
             FormTC, PendingCertificateWritesAuthorized,
             FormedTimeoutCertificatesSound, TcTransportBacked,
             ContextIdentityBindsFrozenEpoch,
             OldContextCertificateRejected, ContextParentWasApplied,
             AllPendingRequests, DecisionAgreement,
             QcValid, CurrentEpoch, Certificate, Request
    <2> QED BY <2>8 DEF StrongInductiveInvariant
  <1> QED BY <1>1

THEOREM DeliverTCPreservesStrongInvariant ==
  \A envelope:
    StrongInductiveInvariant /\ DeliverTC(envelope)
      => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME NEW envelope,
              StrongInductiveInvariant,
              DeliverTC(envelope)
         PROVE StrongInductiveInvariant'
    <2>1. /\ envelope.tc \in formedTCs
          /\ TCValid(envelope.tc)
          /\ TcWellTyped(envelope.tc)
      BY <1>1, Isa
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             ReducerProvenanceInvariant, TcTransportBacked,
             DeliverTC
    <2>2. /\ TypeInvariant'
          /\ TcTransportBacked'
      BY <1>1, <2>1, Isa
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             ReducerProvenanceInvariant, TcTransportBacked,
             DeliverTC, TcAt
    <2>3. /\ Safety'
          /\ ReducerProvenanceInvariant'
          /\ LineageInvariant'
          /\ ContextIdentityBindsFrozenEpoch'
          /\ OldContextCertificateRejected'
          /\ ContextParentWasApplied'
      BY <1>1, <2>2, IsaT(120)
         DEF StrongInductiveInvariant, Safety,
             ReducerProvenanceInvariant, LineageInvariant,
             DeliverTC, TcTransportBacked,
             ContextIdentityBindsFrozenEpoch,
             OldContextCertificateRejected, ContextParentWasApplied,
             QcValid, CurrentEpoch
    <2> QED BY <2>3 DEF StrongInductiveInvariant
  <1> QED BY <1>1

THEOREM BeginInstallTCPreservesStrongInvariant ==
  \A node, tc:
    StrongInductiveInvariant /\ BeginInstallTC(node, tc)
      => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME NEW node, NEW tc,
              StrongInductiveInvariant,
              BeginInstallTC(node, tc)
         PROVE StrongInductiveInvariant'
    <2> DEFINE Request == InstallTcWal(node, tc, FALSE)
    <2>1. /\ tc \in formedTCs
          /\ TCValid(tc)
          /\ tc.votes # {}
          /\ tc.view < MaxView
          /\ tc.view >= nodeView[node]
          /\ TcWellTyped(tc)
      BY <1>1, Isa
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             ReducerProvenanceInvariant, TcTransportBacked,
             FormedTimeoutCertificatesSound, BeginInstallTC,
             TcAt, TCValid
    <2>2. OnePendingPersistencePerNode'
      BY <1>1, PendingNodesAreAllRequestNodes,
         NewRequestPreservesNodeUniqueness, Isa
         DEF StrongInductiveInvariant, Safety,
             OnePendingPersistencePerNode, BeginInstallTC,
             AllPendingRequests, NodeIdle, Request, InstallTcWal
    <2>3. /\ TypeInvariant'
          /\ PendingCertificateWritesAuthorized'
      BY <1>1, <2>1, Isa
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             ReducerProvenanceInvariant,
             PendingCertificateWritesAuthorized,
             BeginInstallTC, Request, InstallTcWal
    <2>4. /\ Safety'
          /\ ReducerProvenanceInvariant'
          /\ LineageInvariant'
          /\ ContextIdentityBindsFrozenEpoch'
          /\ OldContextCertificateRejected'
          /\ ContextParentWasApplied'
      BY <1>1, <2>1, <2>2, <2>3, IsaT(120)
         DEF StrongInductiveInvariant, Safety,
             ReducerProvenanceInvariant, LineageInvariant,
             BeginInstallTC, PendingCertificateWritesAuthorized,
             ContextIdentityBindsFrozenEpoch,
             OldContextCertificateRejected, ContextParentWasApplied,
             AllPendingRequests, QcValid, CurrentEpoch
    <2> QED BY <2>4 DEF StrongInductiveInvariant
  <1> QED BY <1>1

THEOREM PersistInstallTCPreservesStrongInvariant ==
  \A request:
    StrongInductiveInvariant /\ PersistInstallTC(request)
      => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME NEW request,
              StrongInductiveInvariant,
              PersistInstallTC(request)
         PROVE StrongInductiveInvariant'
    <2> DEFINE Node == request.node
    <2> DEFINE Certificate == request.tc
    <2> DEFINE SelectedRank == TcHighRank(Certificate)
    <2> DEFINE SelectedSubject == TcHighSubject(Certificate)
    <2>1. /\ request \in pendingInstallTC
          /\ Certificate \in formedTCs
          /\ TCValid(Certificate)
          /\ Certificate.votes # {}
          /\ Certificate.view < MaxView
          /\ Certificate.view >= nodeView[Node]
      BY <1>1
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             PendingCertificateWritesAuthorized, PersistInstallTC,
             Certificate, Node
    <2>2. /\ SelectedRank \in Ranks
          /\ (SelectedRank = NoRank => SelectedSubject = NoSubject)
          /\ (SelectedRank # NoRank
                => \E qc \in prepareQCs:
                     /\ qc.context = context
                     /\ qc.view = SelectedRank
                     /\ qc.subject = SelectedSubject)
      BY <2>1, IsaT(120)
         DEF TCValid, HighRefValid, TcHighRank, TcHighSubject,
             HighestTimeoutVote, SelectedRank, SelectedSubject,
             Certificate, Ranks
    <2>3. OnePendingPersistencePerNode'
      BY <1>1, RemovingRequestsPreservesNodeUniqueness, Isa
         DEF StrongInductiveInvariant, Safety,
             OnePendingPersistencePerNode, AllPendingRequests,
             PersistInstallTC
    <2>4. /\ TypeInvariant'
          /\ LockBelowHighest'
      BY <1>1, <2>1, <2>2, IsaT(180)
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             LockBelowHighest, PersistInstallTC,
             Node, Certificate, SelectedRank, SelectedSubject,
             Views, Generations, Ranks, ModelConfiguration
    <2>5. HighestAndLockAreCertified'
      BY <1>1, <2>1, <2>2, IsaT(180)
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             HighestAndLockAreCertified, PersistInstallTC,
             Node, Certificate, SelectedRank, SelectedSubject
    <2>6. /\ PendingVoteWritesAuthorized'
          /\ PendingCertificateWritesAuthorized'
      BY <1>1, <2>1, <2>2, IsaT(240)
         DEF StrongInductiveInvariant, Safety,
             OnePendingPersistencePerNode, RequestsUniqueByNode,
             AllPendingRequests, ReducerProvenanceInvariant,
             PendingVoteWritesAuthorized,
             PendingCertificateWritesAuthorized,
             PersistInstallTC, NodeTimedOut,
             PrepareCarriesHigherSafeQc,
             Node, Certificate, SelectedRank, SelectedSubject
    <2>7. /\ TcTransportBacked'
          /\ FormedTimeoutCertificatesSound'
      BY <1>1, <2>1, Isa
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             TcTransportBacked, FormedTimeoutCertificatesSound,
             PersistInstallTC, Node, Certificate,
             BroadcastTCs, TcEnvelope
    <2>8. /\ Safety'
          /\ ReducerProvenanceInvariant'
          /\ LineageInvariant'
          /\ ContextIdentityBindsFrozenEpoch'
          /\ OldContextCertificateRejected'
          /\ ContextParentWasApplied'
      BY <1>1, <2>1, <2>2, <2>3, <2>4, <2>5, <2>6, <2>7,
         IsaT(300)
         DEF StrongInductiveInvariant, Safety,
             ReducerProvenanceInvariant, LineageInvariant,
             PersistInstallTC, PendingVoteWritesAuthorized,
             PendingCertificateWritesAuthorized, TcTransportBacked,
             FormedTimeoutCertificatesSound,
             PrepareLineageSound, LocksCoverOwnCommits,
             CurrentIntentViewsBound, HonestCommitIntentPrepared,
             DurableIntentsDoNotAnticipateHeight,
             ContextIdentityBindsFrozenEpoch,
             OldContextCertificateRejected, ContextParentWasApplied,
             AllPendingRequests, DecisionAgreement,
             QcValid, CurrentEpoch,
             Node, Certificate, SelectedRank, SelectedSubject
    <2> QED BY <2>8 DEF StrongInductiveInvariant
  <1> QED BY <1>1

THEOREM AdvanceContextPreservesStrongInvariant ==
  \A subject:
    StrongInductiveInvariant /\ AdvanceContext(subject)
      => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME NEW subject,
              StrongInductiveInvariant,
              AdvanceContext(subject)
         PROVE StrongInductiveInvariant'
    <2> DEFINE NextHeight == height + 1
    <2> DEFINE NextLineage == Append(context.lineage, subject)
    <2> DEFINE NextContext == ContextRecord(NextHeight, NextLineage)
    <2>1. /\ height \in Heights
          /\ height < MaxHeight
          /\ subject \in Subjects
          /\ context \in ContextRecords
          /\ context.height = height
          /\ NextHeight \in Heights
          /\ NextLineage \in LineagesAt(NextHeight)
          /\ NextContext \in ContextRecords
          /\ NextContext.height = NextHeight
          /\ height' = NextHeight
          /\ context' = NextContext
      BY <1>1, IsaT(180)
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             AdvanceContext, CommonAppliedSubject,
             NextHeight, NextLineage, NextContext,
             ContextRecords, ContextRecord, LineagesAt, Heights
    <2>2. /\ \A vote \in prepareIntents:
               vote.context # NextContext
          /\ \A vote \in commitIntents:
               vote.context # NextContext
          /\ \A vote \in timeoutIntents:
               vote.context # NextContext
      BY <1>1, <2>1, SMT
         DEF StrongInductiveInvariant, LineageInvariant,
             DurableIntentsDoNotAnticipateHeight,
             NextContext, NextHeight, Heights, ContextRecord
    <2>3. (Responsive \cap CurrentVoters) # {}
      <3>1. DualQuorumIntersectionHasHonest
        BY <1>1, DualQuorumHonestIntersection
           DEF StrongInductiveInvariant, Safety, TypeInvariant,
               ModelConfiguration
      <3>2. DualQuorum(CurrentEpoch,
                       Responsive \cap VotingRoster(CurrentEpoch))
        BY <1>1
           DEF StrongInductiveInvariant, Safety, TypeInvariant,
               ModelConfiguration
      <3> QED BY <1>1, <3>1, <3>2, Isa
         DEF DualQuorumIntersectionHasHonest, CurrentVoters,
             CurrentEpoch, ModelConfiguration
    <2>4. PICK witness \in Responsive \cap CurrentVoters: TRUE
      BY <2>3
    <2>5. PICK parentDecision \in decisions:
             /\ parentDecision.node = witness
             /\ parentDecision.qc.context = context
             /\ parentDecision.qc.subject = subject
             /\ [node |-> witness, qc |-> parentDecision.qc] \in applied
      BY <1>1, <2>4
         DEF AdvanceContext, CommonAppliedSubject
    <2>6. TypeInvariant'
      BY <1>1, <2>1, IsaT(300)
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             AdvanceContext, NextHeight, NextLineage, NextContext,
             ContextRecords, ContextRecord, Heights, Views, Generations,
             Ranks, ModelConfiguration
    <2>7. Safety'
      BY <1>1, <2>6, IsaT(240)
         DEF StrongInductiveInvariant, Safety, AdvanceContext,
             OnePendingPersistencePerNode, RequestsUniqueByNode,
             AllPendingRequests, ProposalSigningRequiresIntent,
             PrepareSigningRequiresIntent, CommitSigningRequiresIntent,
             TimeoutSigningRequiresIntent, HonestPrepareUniqueness,
             HonestCommitUniqueness, HonestTimeoutUniqueness,
             LockBelowHighest, DecisionAgreement, AppliedRequiresDecision,
             NoRank
    <2>8. ContextIdentityBindsFrozenEpoch'
      BY <1>1
         DEF ContextIdentityBindsFrozenEpoch
    <2>9. OldContextCertificateRejected'
      BY <1>1, Isa
         DEF AdvanceContext, OldContextCertificateRejected,
             QcValid, CurrentEpoch
    <2>10. ContextParentWasApplied'
      BY <1>1, <2>1, <2>5, IsaT(180)
         DEF StrongInductiveInvariant, ContextParentWasApplied,
             AdvanceContext, NextContext, NextHeight, NextLineage,
             ContextRecord
    <2>11. ReducerProvenanceInvariant'
      BY <1>1, <2>1, IsaT(300)
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             AdvanceContext, HonestVoteUnique, HonestTimeoutUnique,
             IntentPhasesCorrect, PendingVoteWritesAuthorized,
             PendingCertificateWritesAuthorized,
             HonestVoteTransportBacked, QcTransportBacked,
             HonestTimeoutTransportBacked, TcTransportBacked,
             CertificatesBackedByIntents, HonestDurableIntentsSound,
             FormedTimeoutCertificatesSound,
             DurableTimeoutsProtectCommits, HighestAndLockAreCertified,
             VoteIntentFor, NoRank, NoSubject
    <2>12. LineageInvariant'
      BY <1>1, <2>1, <2>2, IsaT(300)
         DEF StrongInductiveInvariant, LineageInvariant,
             AdvanceContext, PrepareLineageSound,
             PrepareCarriesHigherSafeQc, LocksCoverOwnCommits,
             CurrentIntentViewsBound, HonestCommitIntentPrepared,
             CommitIntentsPreparedBy, CertificatePhasesCorrect,
             DurableIntentsDoNotAnticipateHeight,
             NextContext, NextHeight, NoRank
    <2> QED BY <2>7, <2>8, <2>9, <2>10, <2>11, <2>12
       DEF StrongInductiveInvariant
  <1> QED BY <1>1

THEOREM ApplyDecisionPreservesStrongInvariant ==
  \A node \in ValidatorIds:
    \A qc:
      StrongInductiveInvariant /\ ApplyDecision(node, qc)
        => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
              NEW qc,
              StrongInductiveInvariant,
              ApplyDecision(node, qc)
         PROVE StrongInductiveInvariant'
    <2>1. Safety'
      BY <1>1, IsaT(120)
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             OnePendingPersistencePerNode, AllPendingRequests,
             RequestsUniqueByNode, ProposalSigningRequiresIntent,
             PrepareSigningRequiresIntent, CommitSigningRequiresIntent,
             TimeoutSigningRequiresIntent, HonestPrepareUniqueness,
             HonestCommitUniqueness, HonestTimeoutUniqueness,
             LockBelowHighest, DecisionAgreement, AppliedRequiresDecision,
             ApplyDecision
    <2>2. /\ ContextIdentityBindsFrozenEpoch'
          /\ OldContextCertificateRejected'
          /\ ContextParentWasApplied'
      BY <1>1, Isa
         DEF StrongInductiveInvariant, ApplyDecision,
             ContextIdentityBindsFrozenEpoch, OldContextCertificateRejected,
             ContextParentWasApplied, QcValid, CurrentEpoch
    <2>3. ReducerProvenanceInvariant'
      BY <1>1, UnchangedProvenanceVarsPreservesReducerProvenance
         DEF StrongInductiveInvariant, ApplyDecision, ProvenanceVars
    <2> QED BY <1>1, <2>1, <2>2, <2>3,
                  UnchangedLineageVarsPreservesLineageInvariant
       DEF StrongInductiveInvariant, ApplyDecision, LineageVars
  <1> QED BY <1>1

THEOREM NextPreservesStrongInductiveInvariant ==
  StrongInductiveInvariant /\ Next
    => StrongInductiveInvariant'
BY IsaM("blast"),
   SetGSTPreservesStrongInductiveInvariant,
   AssembleLocalBodyPreservesStrongInvariant,
   BeginLocalProposalPreservesStrongInvariant,
   PersistProposalPreservesStrongInvariant,
   CompleteProposalSignaturePreservesStrongInvariant,
   DeliverProposalPreservesStrongInvariant,
   FetchBodyPreservesStrongInvariant,
   StoreBodyPreservesStrongInvariant,
   ValidateBodyPreservesStrongInvariant,
   RejectBodyPreservesStrongInvariant,
   BeginPreparePreservesStrongInvariant,
   PersistPreparePreservesStrongInvariant,
   CompleteVoteSignaturePreservesStrongInvariant,
   ByzantineBroadcastVotePreservesStrongInvariant,
   DeliverVotePreservesStrongInvariant,
   FormPrepareQCPreservesStrongInvariant,
   DeliverQCPreservesStrongInvariant,
   BeginObservePreparePreservesStrongInvariant,
   PersistObservePreparePreservesStrongInvariant,
   BeginLockCommitPreservesStrongInvariant,
   PersistLockCommitPreservesStrongInvariant,
   FormCommitQCPreservesStrongInvariant,
   BeginDecisionPreservesStrongInvariant,
   PersistDecisionPreservesStrongInvariant,
   BeginTimeoutPreservesStrongInvariant,
   PersistTimeoutPreservesStrongInvariant,
   CompleteTimeoutSignaturePreservesStrongInvariant,
   ByzantineBroadcastTimeoutPreservesStrongInvariant,
   DeliverTimeoutPreservesStrongInvariant,
   FormTCPreservesStrongInvariant,
   DeliverTCPreservesStrongInvariant,
   BeginInstallTCPreservesStrongInvariant,
   PersistInstallTCPreservesStrongInvariant,
   FetchCertifiedBodyPreservesStrongInvariant,
   ApplyDecisionPreservesStrongInvariant,
   CrashPreservesStrongInvariant,
   RestartPreservesStrongInvariant,
   ResumeProposalPreservesStrongInvariant,
   ResumeVotePreservesStrongInvariant,
   ResumeTimeoutPreservesStrongInvariant,
   DropProposalPreservesStrongInvariant
   DEF Next

THEOREM NextV2PreservesStrongInductiveInvariant ==
  StrongInductiveInvariant /\ NextV2
    => StrongInductiveInvariant'
BY IsaM("blast"), NextPreservesStrongInductiveInvariant,
   AdvanceContextPreservesStrongInvariant
   DEF NextV2

THEOREM StrongInductiveActionPreservation ==
  StrongInductiveInvariant /\ [NextV2]_vars
    => StrongInductiveInvariant'
PROOF
  <1>1. ASSUME StrongInductiveInvariant,
              [NextV2]_vars
         PROVE StrongInductiveInvariant'
    <2>1. CASE NextV2
      BY <1>1, <2>1, NextV2PreservesStrongInductiveInvariant
    <2>2. CASE UNCHANGED vars
      BY <1>1, <2>2, ProofRelevantStutterPreservesStrongInvariant
         DEF vars, ProofRelevantVars
    <2>3. NextV2 \/ UNCHANGED vars
      BY <1>1
    <2> QED BY <2>1, <2>2, <2>3
  <1> QED BY <1>1

=============================================================================
