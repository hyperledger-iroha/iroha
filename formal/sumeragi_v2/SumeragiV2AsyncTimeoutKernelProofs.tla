---- MODULE SumeragiV2AsyncTimeoutKernelProofs ----
EXTENDS SumeragiV2AsyncInstallRunnerContinuationProofs

(***************************************************************************
Timeout-certificate progress boundary.

The production timeout-vote map retains one receipt per
recipient/context/view/signer.  `TimeoutReceiptSignerUniqueAt` records that
concrete ingress property: together with finite receipt storage it is exactly
the missing bridge from a responsive receipt quorum to the disjoint signer
set required by `TCValid`.  The temporal proof may use these milestones only
after deriving them from `AsyncSpecAt`; none is an additional fairness or
deployment assumption.
***************************************************************************)

TimeoutViewGoal(node, roundView) ==
  nodeView[node] > roundView \/ NodeHasDecision(node)

DurableTimeoutVoteAt(node, roundView) ==
  \E vote \in timeoutIntents:
    /\ vote.signer = node
    /\ vote.context = context
    /\ vote.view = roundView

SentTimeoutVoteAt(signer, recipient, roundView) ==
  \E item \in asyncSentItems:
    /\ item.kind = "TimeoutVote"
    /\ item.source = signer
    /\ item.envelope.recipient = recipient
    /\ item.envelope.vote.view = roundView
    /\ item.envelope.vote.signer = signer

ReceivedTimeoutVoteAt(recipient, signer, roundView) ==
  \E received \in receivedTimeoutVotes:
    /\ received.node = recipient
    /\ received.vote.context = context
    /\ received.vote.view = roundView
    /\ received.vote.signer = signer

TimeoutReceiptSignerUniqueAt(recipient, roundView) ==
  \A left, right \in TimeoutVotesAt(recipient, roundView):
    left.signer = right.signer => left = right

ResponsiveTimeoutReceiptQuorumAt(recipient, roundView) ==
  \A signer \in AsyncCurrentResponsiveVoters:
    ReceivedTimeoutVoteAt(recipient, signer, roundView)

TimeoutSignerMap(votes) == [vote \in votes |-> vote.signer]

THEOREM PersistTimeoutMakesVoteDurable ==
  \A request \in pendingTimeout:
    (StrongInductiveInvariant /\ PersistTimeout(request))
      => DurableTimeoutVoteAt(request.node, request.vote.view)'
BY SMTT(30)
   DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
       PendingVoteWritesAuthorized, PersistTimeout,
       DurableTimeoutVoteAt

THEOREM ExecuteSignTimeoutPublishesEveryRecipient ==
  \A command:
    ExecuteSignTimeout(command)
      => \A recipient \in CurrentVoters:
           SentTimeoutVoteAt(command.node, recipient, command.view)'
BY SMTT(30)
   DEF ExecuteSignTimeout, CommandMatches, CompleteTimeoutSignature,
       PublishControlItems, TimeoutOutbox, SentTimeoutVoteAt,
       AsyncNetworkItem, TimeoutEnvelope

THEOREM ExecuteCoreTimeoutDeliveryRecordsReceipt ==
  \A command:
    (ExecuteCoreDelivery(command) /\ command.kind = "DeliverTimeout")
      => ReceivedTimeoutVoteAt(
           command.node, command.item.envelope.vote.signer,
           command.item.envelope.vote.view)'
BY SMTT(30)
   DEF ExecuteCoreDelivery, DeliverTimeout, ReceivedTimeoutVoteAt,
       TimeoutVoteAt

THEOREM ExecutePersistInstallAdvancesCertifiedView ==
  \A command:
    TypeInvariant /\ ExecutePersistInstall(command)
      => TimeoutViewGoal(command.node, command.view)'
BY SMTT(30)
   DEF ExecutePersistInstall, PersistInstallTC, TimeoutViewGoal

THEOREM TimeoutSignerMapRange ==
  \A votes:
    Range(TimeoutSignerMap(votes)) = TimeoutSignerSet(votes)
PROOF
  <1>1. ASSUME NEW votes
         PROVE Range(TimeoutSignerMap(votes)) = TimeoutSignerSet(votes)
    <2>1. Range(TimeoutSignerMap(votes)) =
             {TimeoutSignerMap(votes)[vote]: vote \in votes}
      BY Isa DEF TimeoutSignerMap, Range
    <2>2. {TimeoutSignerMap(votes)[vote]: vote \in votes} =
             {vote.signer: vote \in votes}
      BY Isa DEF TimeoutSignerMap
    <2> QED BY <2>1, <2>2 DEF TimeoutSignerSet
  <1> QED BY <1>1

THEOREM TimeoutSignerMapSurjects ==
  \A votes:
    TimeoutSignerMap(votes)
      \in Surjection(votes, TimeoutSignerSet(votes))
BY TimeoutSignerMapRange, Fun_RangeProperties
   DEF TimeoutSignerMap

THEOREM UniqueFiniteTimeoutVotesAreDisjoint ==
  \A votes:
    (IsFiniteSet(votes)
      /\ (\A left, right \in votes:
            left.signer = right.signer => left = right))
      => TimeoutVotesDisjoint(votes)
PROOF
  <1>1. ASSUME NEW votes,
                IsFiniteSet(votes)
                  /\ (\A left, right \in votes:
                        left.signer = right.signer => left = right)
         PROVE TimeoutVotesDisjoint(votes)
    <2>1. TimeoutSignerMap(votes)
             \in Surjection(votes, TimeoutSignerSet(votes))
      BY TimeoutSignerMapSurjects
    <2>2. TimeoutSignerMap(votes)
             \in Injection(votes, TimeoutSignerSet(votes))
      BY <1>1, Isa DEF TimeoutSignerMap, Injection
    <2>3. Cardinality(TimeoutSignerSet(votes)) = Cardinality(votes)
      BY <1>1, <2>1, <2>2, FS_Surjection
    <2> QED BY <2>3 DEF TimeoutVotesDisjoint
  <1> QED BY <1>1

THEOREM UniqueFiniteTimeoutReceiptsAreDisjoint ==
  \A recipient \in ValidatorIds, roundView \in Views:
    (IsFiniteSet(TimeoutVotesAt(recipient, roundView))
      /\ TimeoutReceiptSignerUniqueAt(recipient, roundView))
      => TimeoutVotesDisjoint(TimeoutVotesAt(recipient, roundView))
BY UniqueFiniteTimeoutVotesAreDisjoint
   DEF TimeoutReceiptSignerUniqueAt

THEOREM TimeoutPoolMakesVoteSetsFinite ==
  \A recipient \in ValidatorIds, roundView \in Views:
    ReceivedTimeoutVotePoolInvariant
      => IsFiniteSet(TimeoutVotesAt(recipient, roundView))
PROOF
  <1>1. ASSUME NEW recipient \in ValidatorIds,
                NEW roundView \in Views,
                ReceivedTimeoutVotePoolInvariant
         PROVE IsFiniteSet(TimeoutVotesAt(recipient, roundView))
    <2> DEFINE Matching ==
          {entry \in receivedTimeoutVotes:
             /\ entry.node = recipient
             /\ entry.vote.context = context
             /\ entry.vote.view = roundView}
    <2>1. IsFiniteSet(receivedTimeoutVotes)
      BY <1>1 DEF ReceivedTimeoutVotePoolInvariant
    <2>2. Matching \subseteq receivedTimeoutVotes
      BY DEF Matching
    <2>3. IsFiniteSet(Matching)
      BY <2>1, <2>2, Isa
    <2>4. LET voteSet == {entry.vote: entry \in Matching}
           IN IsFiniteSet(voteSet)
      BY <2>3, FS_Image
    <2>5. TimeoutVotesAt(recipient, roundView) =
             {entry.vote: entry \in Matching}
      BY Isa DEF TimeoutVotesAt, Matching
    <2> QED BY <2>4, <2>5
  <1> QED BY <1>1

THEOREM TimeoutPoolMakesSignerSlotsUnique ==
  \A recipient \in ValidatorIds, roundView \in Views:
    ReceivedTimeoutVotePoolInvariant
      => TimeoutReceiptSignerUniqueAt(recipient, roundView)
BY SMTT(30)
   DEF ReceivedTimeoutVotePoolInvariant,
       ReceivedTimeoutVoteSlotsUnique, SameTimeoutVoteSlot,
       TimeoutReceiptSignerUniqueAt, TimeoutVotesAt

THEOREM TimeoutPoolMakesVotesDisjoint ==
  \A recipient \in ValidatorIds, roundView \in Views:
    ReceivedTimeoutVotePoolInvariant
      => TimeoutVotesDisjoint(TimeoutVotesAt(recipient, roundView))
BY TimeoutPoolMakesVoteSetsFinite,
   TimeoutPoolMakesSignerSlotsUnique,
   UniqueFiniteTimeoutReceiptsAreDisjoint

THEOREM ConflictingTimeoutDeliveryDoesNotGrowPool ==
  \A envelope:
    (TimeoutVoteSlotOccupied(envelope.recipient, envelope.vote)
      /\ DeliverTimeout(envelope))
      => receivedTimeoutVotes' = receivedTimeoutVotes
BY SMT DEF DeliverTimeout

THEOREM DeliverTimeoutPreservesSlotUniqueness ==
  \A envelope:
    (ReceivedTimeoutVoteSlotsUnique /\ DeliverTimeout(envelope))
      => ReceivedTimeoutVoteSlotsUnique'
BY SMTT(30)
   DEF DeliverTimeout, TimeoutVoteSlotOccupied,
       ReceivedTimeoutVoteSlotsUnique, SameTimeoutVoteSlot,
       TimeoutVoteAt

THEOREM TypedDeliverTimeoutPreservesPoolInvariant ==
  \A envelope \in TimeoutEnvelopeSet:
    (ReceivedTimeoutVotePoolInvariant /\ DeliverTimeout(envelope))
      => ReceivedTimeoutVotePoolInvariant'
PROOF
  <1>1. ASSUME NEW envelope \in TimeoutEnvelopeSet,
                ReceivedTimeoutVotePoolInvariant,
                DeliverTimeout(envelope)
         PROVE ReceivedTimeoutVotePoolInvariant'
    <2> DEFINE Received ==
          TimeoutVoteAt(envelope.recipient, envelope.vote)
    <2>1. ReceivedTimeoutVoteSlotsUnique'
      <3>1. ReceivedTimeoutVoteSlotsUnique
        BY <1>1 DEF ReceivedTimeoutVotePoolInvariant
      <3>2. DeliverTimeout(envelope)
        BY <1>1
      <3>3. (ReceivedTimeoutVoteSlotsUnique
               /\ DeliverTimeout(envelope))
              => ReceivedTimeoutVoteSlotsUnique'
        BY DeliverTimeoutPreservesSlotUniqueness
      <3> QED BY <3>1, <3>2, <3>3
    <2>2. receivedTimeoutVotes' = receivedTimeoutVotes
             \/ receivedTimeoutVotes' =
                  receivedTimeoutVotes \cup {Received}
      BY <1>1 DEF DeliverTimeout, Received
    <2>3. IsFiniteSet(receivedTimeoutVotes')
      <3>1. IsFiniteSet(receivedTimeoutVotes)
        BY <1>1 DEF ReceivedTimeoutVotePoolInvariant
      <3>2. IsFiniteSet(receivedTimeoutVotes \cup {Received})
        BY <3>1, FS_AddElement
      <3> QED BY <2>2, <3>1, <3>2
    <2>4. /\ Received.node \in ValidatorIds
          /\ Received.vote \in TimeoutVoteRecordSet
          /\ Received.vote.context = context'
          /\ Received.vote.height = height'
          /\ Received.vote.signer \in CurrentVoters'
          /\ AuthenticatedHighRef(
               Received.vote.highRank, Received.vote.highSubject)'
          /\ Received.vote.highRank <= Received.vote.view
      <3>1. /\ Received.node = envelope.recipient
            /\ Received.vote = envelope.vote
        BY DEF Received, TimeoutVoteAt
      <3>2. /\ envelope.recipient \in ValidatorIds
            /\ envelope.vote \in TimeoutVoteRecordSet
        BY <1>1 DEF TimeoutEnvelopeSet
      <3>3. /\ context' = context
            /\ height' = height
            /\ prepareQCs' = prepareQCs
        BY <1>1 DEF DeliverTimeout
      <3>4. /\ envelope.vote.context = context
            /\ envelope.vote.height = height
            /\ envelope.vote.signer \in CurrentVoters
            /\ AuthenticatedHighRef(
                 envelope.vote.highRank,
                 envelope.vote.highSubject)
            /\ envelope.vote.highRank <= envelope.vote.view
        BY <1>1 DEF DeliverTimeout
      <3>5. /\ CurrentVoters' = CurrentVoters
            /\ (AuthenticatedHighRef(
                  envelope.vote.highRank,
                  envelope.vote.highSubject)'
                  <=> AuthenticatedHighRef(
                        envelope.vote.highRank,
                        envelope.vote.highSubject))
        BY <3>3, Isa
           DEF CurrentVoters, CurrentEpoch,
               AuthenticatedHighRef, HighRefValid
      <3> QED BY <3>1, <3>2, <3>3, <3>4, <3>5
    <2>5. \A received \in receivedTimeoutVotes:
             /\ received.node \in ValidatorIds
             /\ received.vote \in TimeoutVoteRecordSet
             /\ received.vote.context = context'
             /\ received.vote.height = height'
             /\ received.vote.signer \in CurrentVoters'
             /\ AuthenticatedHighRef(
                  received.vote.highRank,
                  received.vote.highSubject)'
             /\ received.vote.highRank <= received.vote.view
      <3>1. /\ context' = context
            /\ height' = height
            /\ prepareQCs' = prepareQCs
        BY <1>1 DEF DeliverTimeout
      <3>2. CurrentVoters' = CurrentVoters
        BY <3>1 DEF CurrentVoters, CurrentEpoch
      <3>3. \A highRank, highSubject:
               AuthenticatedHighRef(highRank, highSubject)'
                 <=> AuthenticatedHighRef(highRank, highSubject)
        BY <3>1, Isa DEF AuthenticatedHighRef, HighRefValid
      <3> QED BY <1>1, <3>1, <3>2, <3>3
         DEF ReceivedTimeoutVotePoolInvariant
    <2>6. \A received \in receivedTimeoutVotes':
             /\ received.node \in ValidatorIds
             /\ received.vote \in TimeoutVoteRecordSet
             /\ received.vote.context = context'
             /\ received.vote.height = height'
             /\ received.vote.signer \in CurrentVoters'
             /\ AuthenticatedHighRef(
                  received.vote.highRank,
                  received.vote.highSubject)'
             /\ received.vote.highRank <= received.vote.view
      BY <2>2, <2>4, <2>5, Isa
    <2> QED BY <2>1, <2>3, <2>6
       DEF ReceivedTimeoutVotePoolInvariant
  <1> QED BY <1>1

THEOREM CoreNextKeepsPrepareQcsMonotone ==
  Next => prepareQCs \subseteq prepareQCs'
PROOF
  <1>1. ASSUME Next
         PROVE prepareQCs \subseteq prepareQCs'
    <2>1. CASE SetGST
      BY <2>1, Isa DEF SetGST
    <2>2. CASE \E node \in ValidatorIds, subject \in Subjects:
                  AssembleLocalBody(node, subject)
      BY <2>2, Isa DEF AssembleLocalBody
    <2>3. CASE \E node \in ValidatorIds, subject \in Subjects:
                  BeginLocalProposal(node, subject)
      BY <2>3, Isa DEF BeginLocalProposal
    <2>4. CASE \E request \in pendingProposal: PersistProposal(request)
      BY <2>4, Isa DEF PersistProposal
    <2>5. CASE \E request \in signProposals:
                  CompleteProposalSignature(request)
      BY <2>5, Isa DEF CompleteProposalSignature
    <2>6. CASE \E signer \in ValidatorIds, roundView \in Views,
                  subject \in Subjects,
                  timeoutCertificate \in TimeoutCertificateOptionSet,
                  highestPrepare \in PrepareQcOptionSet:
                  ByzantineBroadcastProposal(
                    signer, roundView, subject,
                    timeoutCertificate, highestPrepare)
      BY <2>6, Isa DEF ByzantineBroadcastProposal
    <2>7. CASE \E envelope \in proposalNetwork:
                  DeliverProposal(envelope)
      BY <2>7, Isa DEF DeliverProposal
    <2>8. CASE \E node \in ValidatorIds,
                  proposal \in SeenProposalValues:
                  FetchBody(node, proposal)
                    \/ RebindRetainedBody(node, proposal)
      BY <2>8, Isa DEF FetchBody, RebindRetainedBody
    <2>9. CASE \E node \in ValidatorIds, roundView \in Views,
                  subject \in Subjects:
                  StoreBody(node, roundView, subject)
      BY <2>9, Isa DEF StoreBody
    <2>10. CASE (\E node \in ValidatorIds,
                         proposal \in SeenProposalValues:
                         ValidateBody(node, proposal)
                           \/ RejectBody(node, proposal))
                    \/ (\E node \in ValidatorIds,
                         qc \in DecisionQcValues:
                         ValidateDecidedBody(node, qc))
                    \/ (\E node \in ValidatorIds,
                         qc \in prepareQCs:
                         ValidateLockedBody(node, qc))
      BY <2>10, Isa
         DEF ValidateBody, ValidateDecidedBody, ValidateLockedBody,
             RejectBody
    <2>11. CASE \E node \in ValidatorIds,
                   proposal \in SeenProposalValues:
                   BeginPrepare(node, proposal)
      BY <2>11, Isa DEF BeginPrepare
    <2>12. CASE \E request \in pendingPrepare: PersistPrepare(request)
      BY <2>12, Isa DEF PersistPrepare
    <2>13. CASE \E request \in signVotes: CompleteVoteSignature(request)
      BY <2>13, Isa DEF CompleteVoteSignature
    <2>14. CASE \E signer \in ValidatorIds, roundView \in Views,
                   phase \in Phases, subject \in Subjects:
                   ByzantineBroadcastVote(
                     signer, roundView, phase, subject)
      BY <2>14, Isa DEF ByzantineBroadcastVote
    <2>15. CASE \E envelope \in voteNetwork: DeliverVote(envelope)
      BY <2>15, Isa DEF DeliverVote
    <2>16. CASE \E node \in ValidatorIds, roundView \in Views,
                   subject \in Subjects:
                   FormPrepareQC(node, roundView, subject)
      BY <2>16, Isa DEF FormPrepareQC
    <2>17. CASE (\E envelope \in QcEnvelopeSet:
                         ImportAuthenticatedCommitCertificate(envelope))
                    \/ (\E envelope \in qcNetwork: DeliverQC(envelope))
      BY <2>17, Isa
         DEF ImportAuthenticatedCommitCertificate, DeliverQC
    <2>18. CASE \E node \in ValidatorIds, qc \in ReceivedQcValues:
                   BeginObservePrepare(node, qc)
      BY <2>18, Isa DEF BeginObservePrepare
    <2>19. CASE \E request \in pendingObservePrepare:
                   PersistObservePrepare(request)
      BY <2>19, Isa DEF PersistObservePrepare
    <2>20. CASE \E node \in ValidatorIds, qc \in LockCommitQcValues:
                   BeginLockCommit(node, qc)
      BY <2>20, Isa DEF BeginLockCommit
    <2>21. CASE \E request \in pendingLockCommit:
                   PersistLockCommit(request)
      BY <2>21, Isa DEF PersistLockCommit
    <2>22. CASE \E node \in ValidatorIds, roundView \in Views,
                   subject \in Subjects:
                   FormCommitQC(node, roundView, subject)
      BY <2>22, Isa DEF FormCommitQC
    <2>23. CASE \E node \in ValidatorIds, qc \in ReceivedQcValues:
                   BeginDecision(node, qc)
      BY <2>23, Isa DEF BeginDecision
    <2>24. CASE \E request \in pendingDecision: PersistDecision(request)
      BY <2>24, Isa DEF PersistDecision
    <2>25. CASE \E node \in ValidatorIds: BeginTimeout(node)
      BY <2>25, Isa DEF BeginTimeout
    <2>26. CASE \E request \in pendingTimeout: PersistTimeout(request)
      BY <2>26, Isa DEF PersistTimeout
    <2>27. CASE \E request \in signTimeouts:
                   CompleteTimeoutSignature(request)
      BY <2>27, Isa DEF CompleteTimeoutSignature
    <2>28. CASE \E signer \in ValidatorIds, roundView \in Views,
                   highestPrepare \in PrepareQcOptionSet:
                   ByzantineBroadcastTimeout(
                    signer, roundView, highestPrepare)
      BY <2>28, Isa DEF ByzantineBroadcastTimeout
    <2>29. CASE \E envelope \in timeoutNetwork: DeliverTimeout(envelope)
      BY <2>29, Isa DEF DeliverTimeout
    <2>31. CASE \E envelope \in tcNetwork: DeliverTC(envelope)
      BY <2>31, Isa DEF DeliverTC
    <2>32. CASE \E node \in ValidatorIds, tc \in ReceivedTcValues:
                   BeginInstallTC(node, tc)
      BY <2>32, Isa DEF BeginInstallTC
    <2>33. CASE \E request \in pendingInstallTC: PersistInstallTC(request)
      BY <2>33, Isa DEF PersistInstallTC
    <2>34. CASE \E node \in ValidatorIds,
                   qc \in DecisionQcValues \cup prepareQCs:
                   FetchCertifiedBody(node, qc)
      BY <2>34, Isa DEF FetchCertifiedBody
    <2>35. CASE \E node \in ValidatorIds, qc \in DecisionQcValues:
                   ApplyDecision(node, qc)
      BY <2>35, Isa DEF ApplyDecision
    <2>36. CASE \E node \in ValidatorIds: Crash(node) \/ Restart(node)
      BY <2>36, Isa DEF Crash, Restart
    <2>37. CASE \E node \in ValidatorIds,
                   proposal \in proposalIntents:
                   ResumeProposal(node, proposal)
      BY <2>37, Isa DEF ResumeProposal
    <2>38. CASE \E node \in ValidatorIds,
                   vote \in prepareIntents \cup commitIntents:
                   ResumeVote(node, vote)
      BY <2>38, Isa DEF ResumeVote
    <2>39. CASE \E node \in ValidatorIds, vote \in timeoutIntents:
                   ResumeTimeout(node, vote)
      BY <2>39, Isa DEF ResumeTimeout
    <2>40. CASE \E envelope \in proposalNetwork: DropProposal(envelope)
      BY <2>40, Isa DEF DropProposal
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4, <2>5, <2>6, <2>7,
                <2>8, <2>9, <2>10, <2>11, <2>12, <2>13, <2>14,
                <2>15, <2>16, <2>17, <2>18, <2>19, <2>20, <2>21,
                <2>22, <2>23, <2>24, <2>25, <2>26, <2>27, <2>28,
                <2>29, <2>31, <2>32, <2>33, <2>34, <2>35,
                <2>36, <2>37, <2>38, <2>39, <2>40
         DEF Next, ByzantineProposalJustificationDomain
  <1> QED BY <1>1

THEOREM AuthenticatedHighRefSurvivesPrepareQcGrowth ==
  \A highRank, highSubject:
    (context' = context
      /\ prepareQCs \subseteq prepareQCs'
      /\ AuthenticatedHighRef(highRank, highSubject))
      => AuthenticatedHighRef(highRank, highSubject)'
BY SMT DEF AuthenticatedHighRef, HighRefValid

THEOREM TimeoutPoolFramePreservesInvariant ==
  (ReceivedTimeoutVotePoolInvariant
    /\ receivedTimeoutVotes' = receivedTimeoutVotes
    /\ context' = context
    /\ height' = height
    /\ prepareQCs \subseteq prepareQCs')
    => ReceivedTimeoutVotePoolInvariant'
PROOF
  <1>1. ASSUME ReceivedTimeoutVotePoolInvariant,
              receivedTimeoutVotes' = receivedTimeoutVotes,
              context' = context,
              height' = height,
              prepareQCs \subseteq prepareQCs'
         PROVE ReceivedTimeoutVotePoolInvariant'
    <2>1. CurrentVoters' = CurrentVoters
      BY <1>1, Isa DEF CurrentVoters, CurrentEpoch
    <2>2. \A highRank, highSubject:
             AuthenticatedHighRef(highRank, highSubject)
               => AuthenticatedHighRef(highRank, highSubject)'
      BY <1>1, AuthenticatedHighRefSurvivesPrepareQcGrowth
    <2> QED BY <1>1, <2>1, <2>2, Isa
         DEF ReceivedTimeoutVotePoolInvariant,
             ReceivedTimeoutVoteSlotsUnique, SameTimeoutVoteSlot
  <1> QED BY <1>1

THEOREM ChangedCoreTimeoutDeliveryIsTyped ==
  \A command:
    (AsyncSchedulerTypeInvariant
      /\ ExecuteCoreDelivery(command)
      /\ receivedTimeoutVotes' # receivedTimeoutVotes)
      => \E envelope \in TimeoutEnvelopeSet: DeliverTimeout(envelope)
PROOF
  <1>1. ASSUME NEW command,
              AsyncSchedulerTypeInvariant,
              ExecuteCoreDelivery(command),
              receivedTimeoutVotes' # receivedTimeoutVotes
         PROVE \E envelope \in TimeoutEnvelopeSet:
                 DeliverTimeout(envelope)
    <2>1. AsyncItemTyped(command.item)
      BY <1>1
         DEF AsyncSchedulerTypeInvariant, AsyncTransportTypeInvariant,
             AsyncTransportContentTypeInvariant,
             AsyncTransportHistoryTypeInvariant, ExecuteCoreDelivery
    <2>2. /\ command.item.kind = "TimeoutVote"
           /\ DeliverTimeout(command.item.envelope)
      BY <1>1, SMT
         DEF ExecuteCoreDelivery, DeliverProposal, DeliverVote, DeliverQC,
             DeliverTC
    <2>3. command.item.envelope \in TimeoutEnvelopeSet
      BY <2>1, <2>2, SMT DEF AsyncItemTyped
    <2> QED BY <2>2, <2>3
  <1> QED BY <1>1

THEOREM RegularCoreCommandKeepsTimeoutPool ==
  \A command:
    RegularCoreCommand(command)
      => receivedTimeoutVotes' = receivedTimeoutVotes
PROOF
  <1>1. ASSUME NEW command, RegularCoreCommand(command)
         PROVE receivedTimeoutVotes' = receivedTimeoutVotes
    <2>1. CASE command.kind = "AssembleBody"
                /\ AssembleLocalBody(command.node, command.subject)
      BY <2>1, Isa DEF AssembleLocalBody
    <2>2. CASE command.kind = "BeginProposal"
                /\ BeginLocalProposal(command.node, command.subject)
      BY <2>2, Isa DEF BeginLocalProposal
    <2>3. CASE command.kind = "PersistProposal"
                /\ \E request \in pendingProposal:
                     /\ CommandMatches(
                          command, request.node, request.proposal.view,
                          request.proposal.subject)
                     /\ PersistProposal(request)
      BY <2>3, Isa DEF PersistProposal
    <2>4. CASE \/ /\ command.kind = "FetchBody"
                         /\ HeldChunksFor(command.node, command.view,
                                           command.subject) = AsyncChunks
                         /\ ~BodyHeldBy(
                               durableBodies, command.node, context,
                               command.view, command.subject)
                         /\ \E retainedProposal \in SeenProposalValues:
                              /\ CommandMatches(
                                   command, command.node,
                                   retainedProposal.view,
                                   retainedProposal.subject)
                              /\ FetchBody(command.node, retainedProposal)
                    \/ /\ command.kind = "RebindRetainedBody"
                         /\ \E proposal \in SeenProposalValues:
                              /\ CommandMatches(
                                   command, command.node, proposal.view,
                                   proposal.subject)
                              /\ RebindRetainedBody(command.node, proposal)
      BY <2>4, Isa DEF FetchBody, RebindRetainedBody
    <2>5. CASE command.kind = "StoreBody"
                /\ StoreBody(command.node, command.view, command.subject)
      BY <2>5, Isa DEF StoreBody
    <2>6. CASE command.kind = "ValidateBody"
                /\ ((\E proposal \in SeenProposalValues:
                       /\ CommandMatches(
                            command, command.node, proposal.view,
                            proposal.subject)
                       /\ ValidateBody(command.node, proposal))
                    \/ (\E qc \in DecisionQcValues:
                          /\ CommandMatches(
                               command, command.node, qc.view, qc.subject)
                          /\ ValidateDecidedBody(command.node, qc))
                    \/ (\E qc \in prepareQCs:
                          /\ CommandMatches(
                               command, command.node, qc.view, qc.subject)
                          /\ ValidateLockedBody(command.node, qc)))
      BY <2>6, Isa
         DEF ValidateBody, ValidateDecidedBody, ValidateLockedBody
    <2>7. CASE command.kind = "BeginPrepare"
                /\ \E proposal \in SeenProposalValues:
                     /\ CommandMatches(
                          command, command.node, proposal.view,
                          proposal.subject)
                     /\ BeginPrepare(command.node, proposal)
      BY <2>7, Isa DEF BeginPrepare
    <2>8. CASE command.kind = "PersistPrepare"
                /\ \E request \in pendingPrepare:
                     /\ CommandMatches(
                          command, request.node, request.vote.view,
                          request.vote.subject)
                     /\ PersistPrepare(request)
      BY <2>8, Isa DEF PersistPrepare
    <2>9. CASE command.kind = "BeginObservePrepare"
                /\ \E qc \in ReceivedQcValues:
                     /\ CommandMatches(
                          command, command.node, qc.view, qc.subject)
                     /\ BeginObservePrepare(command.node, qc)
      BY <2>9, Isa DEF BeginObservePrepare
    <2>10. CASE command.kind = "PersistObservePrepare"
                 /\ \E request \in pendingObservePrepare:
                      /\ CommandMatches(
                           command, request.node, request.qc.view,
                           request.qc.subject)
                      /\ PersistObservePrepare(request)
      BY <2>10, Isa DEF PersistObservePrepare
    <2>11. CASE command.kind = "BeginLockCommit"
                 /\ \E qc \in LockCommitQcValues:
                      /\ CommandMatches(
                           command, command.node, qc.view, qc.subject)
                      /\ BeginLockCommit(command.node, qc)
      BY <2>11, Isa DEF BeginLockCommit
    <2>12. CASE command.kind = "PersistLockCommit"
                 /\ \E request \in pendingLockCommit:
                      /\ CommandMatches(
                           command, request.node, request.qc.view,
                           request.qc.subject)
                      /\ PersistLockCommit(request)
      BY <2>12, Isa DEF PersistLockCommit
    <2>13. CASE command.kind = "FormCommitQC"
                 /\ FormCommitQC(
                      command.node, command.view, command.subject)
      BY <2>13, Isa DEF FormCommitQC
    <2>14. CASE command.kind = "BeginDecision"
                 /\ \E qc \in ReceivedQcValues:
                      /\ CommandMatches(
                           command, command.node, qc.view, qc.subject)
                      /\ BeginDecision(command.node, qc)
      BY <2>14, Isa DEF BeginDecision
    <2>15. CASE command.kind = "PersistTimeout"
                 /\ \E request \in pendingTimeout:
                      /\ CommandMatches(
                           command, request.node, request.vote.view,
                           request.vote.highSubject)
                      /\ PersistTimeout(request)
      BY <2>15, Isa DEF PersistTimeout
    <2>17. CASE command.kind = "BeginInstallTC"
                 /\ \E tc \in ReceivedTcValues:
                      /\ command.node = command.node
                      /\ command.view = tc.view
                      /\ BeginInstallTC(command.node, tc)
      BY <2>17, Isa DEF BeginInstallTC
    <2>18. CASE command.kind = "FetchCertifiedBody"
                 /\ command.item.kind = "CertifiedResponse"
                 /\ command.item.envelope.recipient = command.node
                 /\ command.item.envelope.view = command.view
                 /\ command.item.envelope.subject = command.subject
                 /\ CertifiedResponseCapabilityAuthorized(command.item)
                 /\ AcceptCertifiedResponseCapability(
                      command.node, command.view, command.subject)
      BY <2>18, Isa
         DEF AcceptCertifiedResponseCapability,
             InstallCertifiedBodyEffect
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4, <2>5, <2>6,
                <2>7, <2>8, <2>9, <2>10, <2>11, <2>12, <2>13,
                <2>14, <2>15, <2>17, <2>18
         DEF RegularCoreCommand
  <1> QED BY <1>1

THEOREM ChangedExecuteCommandIsCoreTimeoutDelivery ==
  \A command:
    (ExecuteCommand(command)
      /\ receivedTimeoutVotes' # receivedTimeoutVotes)
      => ExecuteCoreDelivery(command)
PROOF
  <1>1. ASSUME NEW command,
              ExecuteCommand(command),
              receivedTimeoutVotes' # receivedTimeoutVotes
         PROVE ExecuteCoreDelivery(command)
    <2>1. CASE ExecuteRegularCommand(command)
      BY <1>1, <2>1, RegularCoreCommandKeepsTimeoutPool, Isa
         DEF ExecuteRegularCommand
    <2>2. CASE ExecuteSignProposal(command)
      BY <1>1, <2>2, Isa
         DEF ExecuteSignProposal, CompleteProposalSignature
    <2>3. CASE ExecuteSignVote(command)
      BY <1>1, <2>3, Isa DEF ExecuteSignVote, CompleteVoteSignature
    <2>4. CASE ExecuteFormPrepareQC(command)
      BY <1>1, <2>4, Isa DEF ExecuteFormPrepareQC, FormPrepareQC
    <2>5. CASE ExecuteSignTimeout(command)
      BY <1>1, <2>5, Isa
         DEF ExecuteSignTimeout, CompleteTimeoutSignature
    <2>6. CASE ExecutePersistInstall(command)
      BY <1>1, <2>6, Isa DEF ExecutePersistInstall, PersistInstallTC
    <2>7. CASE ExecutePersistDecision(command)
      BY <1>1, <2>7, Isa DEF ExecutePersistDecision, PersistDecision
    <2>8. CASE ExecuteRequestCertifiedBody(command)
      BY <1>1, <2>8, Isa DEF ExecuteRequestCertifiedBody, vars
    <2>9. CASE ExecuteApply(command)
      BY <1>1, <2>9, Isa DEF ExecuteApply, ApplyDecision
    <2>10. CASE ExecuteCoreDelivery(command)
      BY <2>10
    <2>11. CASE ExecuteChunkDelivery(command)
      BY <1>1, <2>11, Isa DEF ExecuteChunkDelivery, vars
    <2>12. CASE ExecuteRejectAuthenticatedJunk(command)
      BY <1>1, <2>12, Isa DEF ExecuteRejectAuthenticatedJunk, vars
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4, <2>5, <2>6,
                <2>7, <2>8, <2>9, <2>10, <2>11, <2>12
         DEF ExecuteCommand
  <1> QED BY <1>1

THEOREM ChangedFifoRuntimeExecutesCommand ==
  \A node:
    (FifoRuntimeStep(node)
      /\ receivedTimeoutVotes' # receivedTimeoutVotes)
      => ExecuteCommand(NextNodeCommand(node))
BY SMTT(30)
   DEF FifoRuntimeStep, DeferCommand, DiscardCommand, vars

THEOREM ChangedDeferredDrainExecutesCommand ==
  \A node:
    (DeferredDrainStep(node)
      /\ receivedTimeoutVotes' # receivedTimeoutVotes)
      => ExecuteCommand(NextDeferredCommand(node))
BY SMTT(30)
   DEF DeferredDrainStep, DiscardCommand, vars

THEOREM ChangedRuntimeStepExecutesCommand ==
  \A node:
    (RuntimeStep(node)
      /\ receivedTimeoutVotes' # receivedTimeoutVotes)
      => \E command: ExecuteCommand(command)
PROOF
  <1>1. ASSUME NEW node,
              RuntimeStep(node),
              receivedTimeoutVotes' # receivedTimeoutVotes
         PROVE \E command: ExecuteCommand(command)
    <2>1. CASE DeferredDrainStep(node)
      <3>1. ExecuteCommand(NextDeferredCommand(node))
        BY <1>1, <2>1, ChangedDeferredDrainExecutesCommand
      <3> QED BY <3>1
    <2>2. CASE DeferredTagStep(node)
      BY <1>1, <2>2, Isa
         DEF DeferredTagStep, DeferredTimeoutStep,
             DeferredRetransmitStep, BeginTimeout, vars
    <2>3. CASE DirectTimeoutStep(node)
      BY <1>1, <2>3, Isa DEF DirectTimeoutStep, BeginTimeout, vars
    <2>4. CASE FifoRuntimeStep(node)
      <3>1. ExecuteCommand(NextNodeCommand(node))
        BY <1>1, <2>4, ChangedFifoRuntimeExecutesCommand
      <3> QED BY <3>1
    <2>5. CASE DirectRetransmitStep(node)
      BY <1>1, <2>5, Isa DEF DirectRetransmitStep, vars
    <2>6. CASE IdleRuntimeStep(node)
      BY <1>1, <2>6, Isa DEF IdleRuntimeStep, vars
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4, <2>5, <2>6
         DEF RuntimeStep
  <1> QED BY <1>1

THEOREM ChangedReplayRunNodeContinuationExecutesCommand ==
  \A node:
    /\ ReplayRunNodeCandidateProducerContinuation(node)
    /\ receivedTimeoutVotes' # receivedTimeoutVotes
    => \E command: ExecuteCommand(command)
PROOF
  <1>1. ASSUME NEW node,
                ReplayRunNodeCandidateProducerContinuation(node),
                receivedTimeoutVotes' # receivedTimeoutVotes
         PROVE \E command: ExecuteCommand(command)
    <2>1. CASE
              AsyncCandidateProducerContinuationExactLocalReplayStep(node)
      BY <1>1, <2>1, Isa
         DEF AsyncCandidateProducerContinuationExactLocalReplayStep, vars
    <2>2. CASE
              AsyncCandidateProducerContinuationReplayTargetOnlyTurn(node)
      BY <1>1, <2>2, Isa
         DEF AsyncCandidateProducerContinuationReplayTargetOnlyTurn, vars
    <2>3. CASE
              AsyncCandidateProducerContinuationExactRuntimeReplayStep(node)
      <3>1. RuntimeStep(node)
        BY <2>3, Isa
           DEF AsyncCandidateProducerContinuationExactRuntimeReplayStep,
               RuntimeStep
      <3> QED BY <1>1, <3>1, ChangedRuntimeStepExecutesCommand
    <2> QED BY <1>1, <2>1, <2>2, <2>3
         DEF ReplayRunNodeCandidateProducerContinuation
  <1> QED BY <1>1

THEOREM ChangedRunNodeWorkExecutesCommand ==
  \A node:
    (RunNodeWork(node)
      /\ receivedTimeoutVotes' # receivedTimeoutVotes)
      => \E command: ExecuteCommand(command)
PROOF
  <1>1. ASSUME NEW node,
              RunNodeWork(node),
              receivedTimeoutVotes' # receivedTimeoutVotes
         PROVE \E command: ExecuteCommand(command)
    <2>0. CASE
            ResolveRunNodeCandidateProducerContinuation(node)
      BY <1>1, <2>0, Isa
         DEF ResolveRunNodeCandidateProducerContinuation, vars
    <2>0p. CASE
             ReplayRunNodeCandidateProducerContinuation(node)
      BY <1>1, <2>0p,
         ChangedReplayRunNodeContinuationExecutesCommand
    <2>1. CASE LocalAdmissionStep(node)
      BY <1>1, <2>1, Isa
         DEF LocalAdmissionStep, AdmitProducerCompletion, AdmitCausalHead,
             UpdateLocalAdmissionMetadata, vars
    <2>2. CASE IngressDrainStep(node)
      BY <1>1, <2>2, Isa
         DEF IngressDrainStep, DrainFairIngressSelected, vars
    <2>3. CASE SerializedRuntimeStep(node)
                  \/ SerializedRuntimePrecedesServeIngressStep(node)
      <3>1. RuntimeStep(node)
        BY <2>3
           DEF SerializedRuntimeStep,
               SerializedRuntimePrecedesServeIngressStep
      <3>2. \E command: ExecuteCommand(command)
        BY <1>1, <3>1, ChangedRuntimeStepExecutesCommand
      <3> QED BY <3>2
    <2>4. CASE AsyncServeIngressTargetOnlyTurn(node)
      BY <1>1, <2>4, Isa
         DEF AsyncServeIngressTargetOnlyTurn, vars
    <2>5. CASE SerializedLocalPrecedesServeIngressStep(node)
      BY <1>1, <2>5, Isa
         DEF SerializedLocalPrecedesServeIngressStep,
             SelectedLocalAdmissionAdvance,
             AdmitProducerCompletion, AdmitCausalHead, vars
    <2> QED BY <1>1, <2>0, <2>0p, <2>1, <2>2, <2>3, <2>4,
                 <2>5
         DEF RunNodeWork
  <1> QED BY <1>1

THEOREM AsyncFaultStepKeepsTimeoutPool ==
  AsyncFaultStep => receivedTimeoutVotes' = receivedTimeoutVotes
PROOF
  <1>1. ASSUME AsyncFaultStep
         PROVE receivedTimeoutVotes' = receivedTimeoutVotes
    <2>1. CASE \E packet \in asyncTransport: PreGstLosePacket(packet)
      BY <2>1, Isa DEF PreGstLosePacket, vars
    <2>2. CASE \E node \in ValidatorIds: PreGstCrash(node)
      BY <2>2, Isa DEF PreGstCrash, Crash
    <2>3. CASE \E source \in AsyncIngressSources,
                  recipient \in ValidatorIds,
                  nonce \in 0..(AsyncIngressCapacity - 1):
                  InjectByzantineNoise(source, recipient, nonce)
      BY <2>3, Isa DEF InjectByzantineNoise, vars
    <2>3c. CASE \E kind \in IngressTransportCompletionKinds,
                   recipient \in ValidatorIds,
                   nonce \in 0..(AsyncIngressCapacity - 1):
                   InjectUntrustedTransportCompletion(
                     kind, recipient, nonce)
      BY <2>3c, Isa
         DEF InjectUntrustedTransportCompletion, vars
    <2>4. CASE \E kind \in {"NormalJunk", "ProgressJunk"},
                  source \in ValidatorIds, recipient \in ValidatorIds,
                  nonce \in 0..(AsyncIngressCapacity - 1):
                  InjectAuthenticatedJunk(kind, source, recipient, nonce)
      BY <2>4, Isa DEF InjectAuthenticatedJunk, vars
    <2>5. CASE \E source \in ValidatorIds, recipient \in ValidatorIds,
                  qc \in commitQCs,
                  nonce \in 0..(AsyncIngressCapacity - 1):
                  InjectByzantineCertifiedRequest(
                    source, recipient, qc, nonce)
      BY <2>5, Isa DEF InjectByzantineCertifiedRequest, vars
    <2>6. CASE \E signer \in ValidatorIds, roundView \in Views,
                  subject \in Subjects,
                  timeoutCertificate \in TimeoutCertificateOptionSet,
                  highestPrepare \in PrepareQcOptionSet:
                  AsyncByzantineProposal(
                    signer, roundView, subject,
                    timeoutCertificate, highestPrepare)
      BY <2>6, Isa
         DEF AsyncByzantineProposal, ByzantineBroadcastProposal
    <2>7. CASE \E signer \in ValidatorIds, roundView \in Views,
                  phase \in Phases, subject \in Subjects:
                  AsyncByzantineVote(signer, roundView, phase, subject)
      BY <2>7, Isa DEF AsyncByzantineVote, ByzantineBroadcastVote
    <2>8. CASE \E signer \in ValidatorIds, roundView \in Views,
                  highestPrepare \in PrepareQcOptionSet:
                  AsyncByzantineTimeout(
                    signer, roundView, highestPrepare)
      BY <2>8, Isa
         DEF AsyncByzantineTimeout, ByzantineBroadcastTimeout
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>3c, <2>4, <2>5,
                <2>6, <2>7, <2>8
         DEF AsyncFaultStep, ByzantineProposalJustificationDomain
  <1> QED BY <1>1

THEOREM AsyncNonRunnerStepKeepsTimeoutPool ==
  AsyncNonRunnerStep => receivedTimeoutVotes' = receivedTimeoutVotes
PROOF
  <1>1. ASSUME AsyncNonRunnerStep
         PROVE receivedTimeoutVotes' = receivedTimeoutVotes
    <2>1. CASE AsyncSetGST
      BY <2>1, Isa DEF AsyncSetGST, SetGST
    <2>2. CASE AsyncTick
      BY <2>2, Isa DEF AsyncTick, AsyncNonClockVars, vars
    <2>3. CASE \E node \in ValidatorIds:
                  OpenHistoricalRecovery(node)
      BY <2>3, Isa DEF OpenHistoricalRecovery, vars
    <2>4. CASE \E node \in AsyncCurrentResponsiveVoters:
                  DirectCommitCertificateDiscoveryStep(node)
      BY <2>4, Isa DEF DirectCommitCertificateDiscoveryStep, vars
    <2>5. CASE \E node \in asyncHistoricalRecoveryTargets:
                  DirectHistoricalCommitCertificateDiscoveryStep(node)
      BY <2>5, Isa
         DEF DirectHistoricalCommitCertificateDiscoveryStep,
             CommitCertificateDiscoveryStepWork, vars
    <2>6. CASE \E node \in AsyncArchiveIoServiceNodes:
                  ServiceIoWorker(node)
      BY <2>6, Isa DEF ServiceIoWorker, ServiceIoWorkerWork, vars
    <2>7. CASE \E node \in asyncHistoricalRecoveryTargets:
                  ServiceHistoricalRecoveryIoWorker(node)
      BY <2>7, Isa
         DEF ServiceHistoricalRecoveryIoWorker, ServiceIoWorkerWork, vars
    <2>8. CASE \E node \in AsyncCurrentResponsiveVoters:
                  EnqueueIoLocalControl(node)
      BY <2>8, Isa
         DEF EnqueueIoLocalControl, EnqueueIoLocalControlWork, vars
    <2>9. CASE \E node \in asyncHistoricalRecoveryTargets:
                  EnqueueHistoricalRecoveryIoLocalControl(node)
      BY <2>9, Isa
         DEF EnqueueHistoricalRecoveryIoLocalControl,
             EnqueueIoLocalControlWork, vars
    <2>10. CASE AsyncNetworkStep
      BY <2>10, Isa
         DEF AsyncNetworkStep, AdmitIngressPacket,
             AdmitHiddenPacket, CoalesceHiddenPacket, vars
    <2>11. CASE AsyncFaultStep
      BY <2>11, AsyncFaultStepKeepsTimeoutPool
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4, <2>5, <2>6,
                <2>7, <2>8, <2>9, <2>10, <2>11
         DEF AsyncNonRunnerStep
  <1> QED BY <1>1

THEOREM ChangedAsyncRunnerExecutesCommand ==
  (AsyncRunnerStep
    /\ receivedTimeoutVotes' # receivedTimeoutVotes)
    => \E command: ExecuteCommand(command)
PROOF
  <1>1. ASSUME AsyncRunnerStep,
              receivedTimeoutVotes' # receivedTimeoutVotes
         PROVE \E command: ExecuteCommand(command)
    <2>1. CASE \E node \in AsyncCurrentResponsiveVoters: RunNode(node)
      <3>1. ASSUME NEW node \in AsyncCurrentResponsiveVoters,
                    RunNode(node)
             PROVE \E command: ExecuteCommand(command)
        BY <1>1, <3>1, ChangedRunNodeWorkExecutesCommand
      <3> QED BY <2>1, <3>1
    <2>2. CASE \E node \in asyncHistoricalRecoveryTargets:
                  RunHistoricalRecoveryNode(node)
      <3>1. ASSUME NEW node \in asyncHistoricalRecoveryTargets,
                    RunHistoricalRecoveryNode(node)
             PROVE \E command: ExecuteCommand(command)
        BY <1>1, <3>1, ChangedRunNodeWorkExecutesCommand
           DEF RunHistoricalRecoveryNode
      <3> QED BY <2>2, <3>1
    <2>3. CASE \E node \in AsyncResponsiveAppliedArchiveServers:
                  RunHistoricalServer(node)
      BY <1>1, <2>3, Isa
         DEF RunHistoricalServer, DrainHistoricalIngressSelected,
             HistoricalIdleStep, vars
    <2> QED BY <1>1, <2>1, <2>2, <2>3 DEF AsyncRunnerStep
  <1> QED BY <1>1

THEOREM ChangedAsyncNextExecutesCommandOrCrashes ==
  (AsyncNext
    /\ receivedTimeoutVotes' # receivedTimeoutVotes)
    => \/ \E command: ExecuteCommand(command)
       \/ \E node \in ValidatorIds: Crash(node)
PROOF
  <1>1. ASSUME AsyncNext,
              receivedTimeoutVotes' # receivedTimeoutVotes
         PROVE \/ \E command: ExecuteCommand(command)
               \/ \E node \in ValidatorIds: Crash(node)
    <2>1. CASE AsyncNonCrashStep
      <3>1. CASE AsyncRunnerStep
        BY <1>1, <3>1, ChangedAsyncRunnerExecutesCommand
      <3>2. CASE AsyncNonRunnerStep
        BY <1>1, <3>2, AsyncNonRunnerStepKeepsTimeoutPool
      <3>3. CASE DriveResponsiveReplayHead \/ FinishResponsiveReplay
        BY <1>1, <3>3, Isa
           DEF DriveResponsiveReplayHead, FinishResponsiveReplay,
               RecoveryCoreReplay, ResumeProposal, ResumeVote,
               ResumeTimeout
      <3>4. CASE RearmResponsiveRecovery
        BY <1>1, <3>4, Isa DEF RearmResponsiveRecovery
      <3> QED BY <2>1, <3>1, <3>2, <3>3, <3>4
           DEF AsyncNonCrashStep
    <2>2. CASE \E node \in ValidatorIds: PreGstCrash(node)
      BY <2>2 DEF PreGstCrash
    <2>3. CASE \E node \in ValidatorIds: PreGstResponsiveCrash(node)
      BY <2>3 DEF PreGstResponsiveCrash
    <2>4. CASE PreGstResponsiveRestart
      BY <1>1, <2>4, Isa
         DEF PreGstResponsiveRestart, Restart,
             AsyncSchedulerVars
    <2>5. CASE PreGstResponsiveReplay
      BY <1>1, <2>5, Isa
         DEF PreGstResponsiveReplay, RecoveryCoreReplay,
             ResumeProposal, ResumeVote, ResumeTimeout,
             ResetNodeSchedulerForRestart
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4, <2>5 DEF AsyncNext
  <1> QED BY <1>1

THEOREM ChangedTypedNonCrashAsyncNextIsTimeoutDelivery ==
  (AsyncTypeInvariant
    /\ AsyncNext
    /\ ~(\E node \in ValidatorIds: Crash(node))
    /\ receivedTimeoutVotes' # receivedTimeoutVotes)
    => \E envelope \in TimeoutEnvelopeSet: DeliverTimeout(envelope)
PROOF
  <1>1. ASSUME AsyncTypeInvariant,
              AsyncNext,
              ~(\E node \in ValidatorIds: Crash(node)),
              receivedTimeoutVotes' # receivedTimeoutVotes
         PROVE \E envelope \in TimeoutEnvelopeSet: DeliverTimeout(envelope)
    <2>1. \E command: ExecuteCommand(command)
      BY <1>1, ChangedAsyncNextExecutesCommandOrCrashes
    <2>2. ASSUME NEW command, ExecuteCommand(command)
           PROVE \E envelope \in TimeoutEnvelopeSet:
                   DeliverTimeout(envelope)
      <3>1. ExecuteCoreDelivery(command)
        BY <1>1, <2>2, ChangedExecuteCommandIsCoreTimeoutDelivery
      <3>2. AsyncSchedulerTypeInvariant
        BY <1>1 DEF AsyncTypeInvariant
      <3> QED BY <1>1, <3>1, <3>2,
                    ChangedCoreTimeoutDeliveryIsTyped
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM TimeoutPoolFilteredSubsetPreservesInvariant ==
  /\ ReceivedTimeoutVotePoolInvariant
  /\ receivedTimeoutVotes' \subseteq receivedTimeoutVotes
  /\ context' = context
  /\ height' = height
  /\ prepareQCs' = prepareQCs
  => ReceivedTimeoutVotePoolInvariant'
PROOF
  <1>1. ASSUME ReceivedTimeoutVotePoolInvariant,
              receivedTimeoutVotes' \subseteq receivedTimeoutVotes,
              context' = context,
              height' = height,
              prepareQCs' = prepareQCs
         PROVE ReceivedTimeoutVotePoolInvariant'
    <2>1. /\ IsFiniteSet(receivedTimeoutVotes)
           /\ IsFiniteSet(receivedTimeoutVotes')
      BY <1>1, FS_Subset DEF ReceivedTimeoutVotePoolInvariant
    <2>2. CurrentVoters' = CurrentVoters
      BY <1>1, Isa DEF CurrentVoters, CurrentEpoch
    <2>3. ReceivedTimeoutVoteSlotsUnique'
      <3>1. ASSUME NEW left \in receivedTimeoutVotes',
                    NEW right \in receivedTimeoutVotes',
                    SameTimeoutVoteSlot(left, right)'
             PROVE left = right
        <4>1. /\ left \in receivedTimeoutVotes
               /\ right \in receivedTimeoutVotes
               /\ SameTimeoutVoteSlot(left, right)
          BY <1>1, <3>1, Isa DEF SameTimeoutVoteSlot
        <4> QED BY <1>1, <4>1
             DEF ReceivedTimeoutVotePoolInvariant,
                 ReceivedTimeoutVoteSlotsUnique
      <3> QED BY <3>1 DEF ReceivedTimeoutVoteSlotsUnique
    <2>4. \A received \in receivedTimeoutVotes':
             /\ received.node \in ValidatorIds
             /\ received.vote \in TimeoutVoteRecordSet
             /\ received.vote.context = context'
             /\ received.vote.height = height'
             /\ received.vote.signer \in CurrentVoters'
             /\ AuthenticatedHighRef(
                  received.vote.highRank, received.vote.highSubject)'
             /\ received.vote.highRank <= received.vote.view
      <3>1. ASSUME NEW received \in receivedTimeoutVotes'
             PROVE /\ received.node \in ValidatorIds
                   /\ received.vote \in TimeoutVoteRecordSet
                   /\ received.vote.context = context'
                   /\ received.vote.height = height'
                   /\ received.vote.signer \in CurrentVoters'
                   /\ AuthenticatedHighRef(
                        received.vote.highRank,
                        received.vote.highSubject)'
                   /\ received.vote.highRank <= received.vote.view
        <4>1. received \in receivedTimeoutVotes
          BY <1>1, <3>1
        <4>2. /\ received.node \in ValidatorIds
               /\ received.vote \in TimeoutVoteRecordSet
               /\ received.vote.context = context
               /\ received.vote.height = height
               /\ received.vote.signer \in CurrentVoters
               /\ AuthenticatedHighRef(
                    received.vote.highRank, received.vote.highSubject)
               /\ received.vote.highRank <= received.vote.view
          BY <1>1, <4>1 DEF ReceivedTimeoutVotePoolInvariant
        <4>3. AuthenticatedHighRef(
                 received.vote.highRank, received.vote.highSubject)'
          BY <1>1, <4>2, Isa DEF AuthenticatedHighRef, HighRefValid
        <4> QED BY <1>1, <2>2, <4>2, <4>3
      <3> QED BY <3>1
    <2> QED BY <2>1, <2>3, <2>4
         DEF ReceivedTimeoutVotePoolInvariant
  <1> QED BY <1>1

THEOREM CrashPreservesTimeoutPoolInvariant ==
  \A node \in ValidatorIds:
    /\ ReceivedTimeoutVotePoolInvariant
    /\ Crash(node)
    => ReceivedTimeoutVotePoolInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                ReceivedTimeoutVotePoolInvariant,
                Crash(node)
         PROVE ReceivedTimeoutVotePoolInvariant'
    <2>1. /\ receivedTimeoutVotes' =
                  {entry \in receivedTimeoutVotes: entry.node # node}
           /\ context' = context
           /\ height' = height
           /\ prepareQCs' = prepareQCs
      BY <1>1 DEF Crash
    <2>2. receivedTimeoutVotes' \subseteq receivedTimeoutVotes
      BY <2>1
    <2> QED BY <1>1, <2>1, <2>2,
                TimeoutPoolFilteredSubsetPreservesInvariant
  <1> QED BY <1>1

THEOREM AsyncNextPreservesTimeoutPoolInvariant ==
  AsyncTypeInvariant /\ AsyncNext
    => ReceivedTimeoutVotePoolInvariant'
PROOF
  <1>1. ASSUME AsyncTypeInvariant, AsyncNext
         PROVE ReceivedTimeoutVotePoolInvariant'
    <2>1. ReceivedTimeoutVotePoolInvariant
      BY <1>1 DEF AsyncTypeInvariant
    <2>2. /\ context' = context
           /\ height' = height
      BY <1>1 DEF AsyncNext
    <2>3. [Next]_vars
      BY <1>1 DEF AsyncNext
    <2>4. prepareQCs \subseteq prepareQCs'
      <3>1. CASE Next
        BY <3>1, CoreNextKeepsPrepareQcsMonotone
      <3>2. CASE UNCHANGED vars
        BY <3>2, Isa DEF vars
      <3> QED BY <2>3, <3>1, <3>2
    <2>5. CASE receivedTimeoutVotes' = receivedTimeoutVotes
      BY <2>1, <2>2, <2>4, <2>5,
         TimeoutPoolFramePreservesInvariant
    <2>6. CASE receivedTimeoutVotes' # receivedTimeoutVotes
      <3>1. CASE \E node \in ValidatorIds: Crash(node)
        BY <2>1, <3>1, CrashPreservesTimeoutPoolInvariant
      <3>2. CASE ~(\E node \in ValidatorIds: Crash(node))
        <4>1. \E envelope \in TimeoutEnvelopeSet:
                 DeliverTimeout(envelope)
          BY <1>1, <2>6, <3>2,
             ChangedTypedNonCrashAsyncNextIsTimeoutDelivery
        <4>2. ASSUME NEW envelope \in TimeoutEnvelopeSet,
                      DeliverTimeout(envelope)
               PROVE ReceivedTimeoutVotePoolInvariant'
          BY <2>1, <4>2, TypedDeliverTimeoutPreservesPoolInvariant
        <4> QED BY <4>1, <4>2
      <3> QED BY <3>1, <3>2
    <2> QED BY <2>5, <2>6
  <1> QED BY <1>1

THEOREM DualQuorumMonotoneWithinRoster ==
  \A epoch \in Epochs:
    \A left, right \in SUBSET VotingRoster(epoch):
      (QuorumConfiguration
        /\ left \subseteq right
        /\ DualQuorum(epoch, left))
        => DualQuorum(epoch, right)
PROOF
  <1>1. ASSUME NEW epoch \in Epochs,
              NEW left \in SUBSET VotingRoster(epoch),
              NEW right \in SUBSET VotingRoster(epoch),
              QuorumConfiguration,
              left \subseteq right,
              DualQuorum(epoch, left)
         PROVE DualQuorum(epoch, right)
    <2>1. /\ IsFiniteSet(VotingRoster(epoch))
           /\ left \subseteq VotingRoster(epoch)
           /\ right \subseteq VotingRoster(epoch)
      BY <1>1 DEF QuorumConfiguration
    <2>2. /\ IsFiniteSet(left)
           /\ IsFiniteSet(right)
      BY <2>1, FS_Subset
    <2>3. /\ Cardinality(left) \in Nat
           /\ Cardinality(right) \in Nat
           /\ Cardinality(VotingRoster(epoch)) \in Nat
           /\ Cardinality(left) <= Cardinality(right)
      BY <1>1, <2>1, <2>2, FS_CardinalityType, FS_Subset
    <2>4. CountQuorum(epoch, right)
      BY <1>1, <2>1, <2>3, SMT
         DEF DualQuorum, CountQuorum
    <2>5. /\ VotingRoster(epoch) \in SUBSET ValidatorIds
           /\ left \in SUBSET ValidatorIds
           /\ right \in SUBSET ValidatorIds
      BY <1>1, <2>1, Isa DEF QuorumConfiguration, VotingRoster
    <2>6. PowerUnits(epoch, left)
             \subseteq PowerUnits(epoch, right)
      BY <1>1, <2>5, PowerUnitsMonotone
    <2>7. /\ IsFiniteSet(PowerUnits(epoch, left))
           /\ IsFiniteSet(PowerUnits(epoch, right))
           /\ IsFiniteSet(PowerUnits(epoch, VotingRoster(epoch)))
      BY <1>1, PowerUnitsFinite
    <2>8. /\ Cardinality(PowerUnits(epoch, left)) \in Nat
           /\ Cardinality(PowerUnits(epoch, right)) \in Nat
           /\ Cardinality(PowerUnits(epoch, VotingRoster(epoch))) \in Nat
           /\ Cardinality(PowerUnits(epoch, left))
                <= Cardinality(PowerUnits(epoch, right))
      BY <2>6, <2>7, FS_CardinalityType, FS_Subset
    <2>9. PowerQuorum(epoch, right)
      BY <1>1, <2>1, <2>8, SMT
         DEF DualQuorum, PowerQuorum, PowerOf
    <2> QED BY <2>4, <2>9 DEF DualQuorum
  <1> QED BY <1>1

THEOREM ResponsiveReceiptsCoverResponsiveSigners ==
  \A recipient \in ValidatorIds, roundView \in Views:
    ResponsiveTimeoutReceiptQuorumAt(recipient, roundView)
      => AsyncCurrentResponsiveVoters
           \subseteq TimeoutSignerSet(
             TimeoutVotesAt(recipient, roundView))
BY SMTT(30)
   DEF ResponsiveTimeoutReceiptQuorumAt, ReceivedTimeoutVoteAt,
       AsyncCurrentResponsiveVoters, TimeoutSignerSet, TimeoutVotesAt

THEOREM TimeoutPoolSignersStayInCurrentRoster ==
  \A recipient \in ValidatorIds, roundView \in Views:
    ReceivedTimeoutVotePoolInvariant
      => TimeoutSignerSet(TimeoutVotesAt(recipient, roundView))
           \subseteq CurrentVoters
BY SMTT(30)
   DEF ReceivedTimeoutVotePoolInvariant,
       TimeoutSignerSet, TimeoutVotesAt

THEOREM TypeInvariantMakesCurrentEpochTyped ==
  TypeInvariant => CurrentEpoch \in Epochs
PROOF
  <1>1. ASSUME TypeInvariant
         PROVE CurrentEpoch \in Epochs
    <2>1. /\ ModelConfiguration
           /\ context \in ContextRecords
      BY <1>1 DEF TypeInvariant
    <2>2. PICK blockHeight \in Heights:
             \E lineage \in LineagesAt(blockHeight):
               context = ContextRecord(blockHeight, lineage)
      BY <2>1, Isa DEF ContextRecords
    <2>3. PICK lineage \in LineagesAt(blockHeight):
             context = ContextRecord(blockHeight, lineage)
      BY <2>2
    <2>4. /\ context.epoch = ExpectedEpoch(blockHeight)
           /\ MaxEpoch >= ExpectedEpoch(MaxHeight)
      BY <2>1, <2>3
         DEF ContextRecord, ModelConfiguration
    <2>5. /\ blockHeight \in Nat
           /\ blockHeight <= MaxHeight
           /\ MaxHeight \in Nat
           /\ EpochLength \in Nat \ {0}
           /\ MaxEpoch \in Nat
      BY <2>1, <2>2, SMT
         DEF Heights, ModelConfiguration, QuorumConfiguration
    <2>6. ExpectedEpoch(blockHeight) \in 0..MaxEpoch
      BY <2>4, <2>5, BoundedNaturalQuotient DEF ExpectedEpoch
    <2> QED BY <2>4, <2>6 DEF CurrentEpoch, Epochs
  <1> QED BY <1>1

THEOREM ResponsiveReceiptsMakeDualQuorum ==
  \A recipient \in ValidatorIds, roundView \in Views:
    (TypeInvariant
      /\ ReceivedTimeoutVotePoolInvariant
      /\ ResponsiveTimeoutReceiptQuorumAt(recipient, roundView))
      => DualQuorum(
           CurrentEpoch,
           TimeoutSignerSet(TimeoutVotesAt(recipient, roundView)))
PROOF
  <1>1. ASSUME NEW recipient \in ValidatorIds,
              NEW roundView \in Views,
              TypeInvariant,
              ReceivedTimeoutVotePoolInvariant,
              ResponsiveTimeoutReceiptQuorumAt(recipient, roundView)
         PROVE DualQuorum(
                 CurrentEpoch,
                 TimeoutSignerSet(TimeoutVotesAt(recipient, roundView)))
    <2> DEFINE Signers ==
          TimeoutSignerSet(TimeoutVotesAt(recipient, roundView))
    <2>1. /\ ModelConfiguration
           /\ QuorumConfiguration
      BY <1>1 DEF TypeInvariant, ModelConfiguration
    <2>2. CurrentEpoch \in Epochs
      BY <1>1, TypeInvariantMakesCurrentEpochTyped
    <2>3. /\ CurrentVoters = VotingRoster(CurrentEpoch)
           /\ DualQuorum(CurrentEpoch, AsyncCurrentResponsiveVoters)
      BY <2>1, <2>2, Isa
         DEF ModelConfiguration, CurrentVoters,
             AsyncCurrentResponsiveVoters
    <2>4. AsyncCurrentResponsiveVoters \subseteq Signers
      BY <1>1, ResponsiveReceiptsCoverResponsiveSigners DEF Signers
    <2>5. Signers \subseteq CurrentVoters
      BY <1>1, TimeoutPoolSignersStayInCurrentRoster DEF Signers
    <2>6. /\ AsyncCurrentResponsiveVoters
                  \in SUBSET VotingRoster(CurrentEpoch)
           /\ Signers \in SUBSET VotingRoster(CurrentEpoch)
      BY <2>3, <2>5, Isa
    <2> QED BY <2>1, <2>2, <2>3, <2>4, <2>6,
                  DualQuorumMonotoneWithinRoster
  <1> QED BY <1>1

THEOREM AsyncInitEstablishesTimeoutPoolInvariant ==
  \A initialContext:
    AsyncInitAt(initialContext) => ReceivedTimeoutVotePoolInvariant
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncInitAt(initialContext)
         PROVE ReceivedTimeoutVotePoolInvariant
    <2>1. receivedTimeoutVotes = {}
      BY <1>1 DEF AsyncInitAt, AsyncBaseInitAt, InitAt
    <2>2. IsFiniteSet(receivedTimeoutVotes)
      BY <2>1, FS_EmptySet
    <2>3. ReceivedTimeoutVoteSlotsUnique
      BY <2>1 DEF ReceivedTimeoutVoteSlotsUnique
    <2>4. \A received \in receivedTimeoutVotes:
             /\ received.node \in ValidatorIds
             /\ received.vote \in TimeoutVoteRecordSet
             /\ received.vote.context = context
             /\ received.vote.height = height
             /\ received.vote.signer \in CurrentVoters
             /\ AuthenticatedHighRef(
                  received.vote.highRank,
                  received.vote.highSubject)
             /\ received.vote.highRank <= received.vote.view
      BY <2>1
    <2> QED BY <2>2, <2>3, <2>4
       DEF ReceivedTimeoutVotePoolInvariant
  <1> QED BY <1>1

(***************************************************************************
Non-runner scheduler closure.  Fault injections may extend only the
transport-history slice; this projection lemma reuses the primitive stutter
proofs for every other scheduler component.
***************************************************************************)

THEOREM AsyncTransportContentChangePreservesSchedulerType ==
  /\ AsyncSchedulerTypeInvariant
  /\ AsyncTransportContentTypeInvariant'
  /\ UNCHANGED AsyncRuntimeScalarTypeVars
  /\ UNCHANGED asyncCausalQueues
  /\ UNCHANGED AsyncIoTopologyTypeVars
  /\ UNCHANGED AsyncIoContentTypeVars
  /\ UNCHANGED AsyncIoCapacityTypeVars
  /\ UNCHANGED AsyncDeferredTopologyTypeVars
  /\ UNCHANGED <<asyncDeferredCompletionQueues,
                  asyncDeferredProgressQueues,
                  asyncDeferredNormalQueues>>
  /\ UNCHANGED AsyncTransportClockTypeVars
  /\ UNCHANGED AsyncIngressTopologyTypeVars
  /\ UNCHANGED asyncIngressLanes
  /\ AsyncHistoricalRecoveryTypeInvariant'
  => AsyncSchedulerTypeInvariant'
BY AsyncRuntimeScalarTypeStutter, AsyncCausalTypeStutter,
   AsyncIoTopologyTypeStutter, AsyncIoContentTypeStutter,
   AsyncIoCapacityTypeStutter, AsyncDeferredTopologyTypeStutter,
   AsyncDeferredContentTypeStutter, AsyncTransportClockTypeStutter,
   AsyncIngressTopologyTypeStutter, AsyncIngressCapacityTypeStutter,
   AsyncIngressContentTypeStutter
   DEF AsyncSchedulerTypeInvariant, AsyncRuntimeTypeInvariant,
       AsyncIoTypeInvariant, AsyncDeferredTypeInvariant,
       AsyncTransportTypeInvariant, AsyncIngressTypeInvariant

THEOREM AddTypedPacketPreservesPacketContentType ==
  \A packet:
    /\ AsyncPacketContentTypeInvariant
    /\ AsyncPacketTyped(packet)
    /\ asyncTransport' = asyncTransport \cup {packet}
    => AsyncPacketContentTypeInvariant'
PROOF
  <1>1. ASSUME NEW packet,
                AsyncPacketContentTypeInvariant,
                AsyncPacketTyped(packet),
                asyncTransport' = asyncTransport \cup {packet}
         PROVE AsyncPacketContentTypeInvariant'
    <2>1. IsFiniteSet(asyncTransport')
      BY <1>1, FS_AddElement DEF AsyncPacketContentTypeInvariant
    <2>2. \A queued \in asyncTransport': AsyncPacketTyped(queued)
      BY <1>1 DEF AsyncPacketContentTypeInvariant
    <2> QED BY <2>1, <2>2 DEF AsyncPacketContentTypeInvariant
  <1> QED BY <1>1

THEOREM AddUntrackedTypedPacketPreservesTransportContentType ==
  \A packet:
    /\ AsyncTransportContentTypeInvariant
    /\ AsyncPacketTyped(packet)
    /\ asyncTransport' = asyncTransport \cup {packet}
    /\ UNCHANGED AsyncTransportHistoryTypeVars
    /\ UNCHANGED asyncHeldChunks
    => AsyncTransportContentTypeInvariant'
BY AddTypedPacketPreservesPacketContentType,
   AsyncTransportHistoryTypeStutter, AsyncHeldChunksTypeStutter
   DEF AsyncTransportContentTypeInvariant

THEOREM AsyncHeartbeatSubjectIsValid ==
  ModelConfiguration => AsyncHeartbeatSubject \in ValidSubjects
BY SMT DEF ModelConfiguration, AsyncHeartbeatSubject

THEOREM AsyncNoiseItemIsTyped ==
  \A source \in AsyncIngressSources, recipient \in ValidatorIds,
     nonce \in 0..(AsyncIngressCapacity - 1):
    LET envelope ==
          AsyncBodyEnvelope(recipient, context.height,
                            nodeView[recipient],
                            AsyncHeartbeatSubject, NoAsyncChunk, nonce)
        item == AsyncNetworkItem("Noise", source, envelope)
    IN /\ TypeInvariant
       /\ AsyncConfiguration
       => AsyncItemTyped(item)
PROOF
  <1>1. ASSUME NEW source \in AsyncIngressSources,
                NEW recipient \in ValidatorIds,
                NEW nonce \in 0..(AsyncIngressCapacity - 1)
         PROVE LET envelope ==
                 AsyncBodyEnvelope(recipient, context.height,
                                   nodeView[recipient],
                                   AsyncHeartbeatSubject,
                                   NoAsyncChunk, nonce)
               item == AsyncNetworkItem("Noise", source, envelope)
               IN /\ TypeInvariant
                  /\ AsyncConfiguration
                  => AsyncItemTyped(item)
    <2>1. ASSUME TypeInvariant, AsyncConfiguration
           PROVE LET envelope ==
                   AsyncBodyEnvelope(recipient, context.height,
                                     nodeView[recipient],
                                     AsyncHeartbeatSubject,
                                     NoAsyncChunk, nonce)
                 item == AsyncNetworkItem("Noise", source, envelope)
                 IN AsyncItemTyped(item)
      <3>1. /\ ModelConfiguration
             /\ context.height \in Heights
             /\ nodeView[recipient] \in Views
             /\ AsyncHeartbeatSubject \in ValidSubjects
        BY <1>1, <2>1, AsyncHeartbeatSubjectIsValid
           DEF TypeInvariant
      <3> QED BY <1>1, <2>1, <3>1, SMT
           DEF AsyncItemTyped, AsyncNetworkItem,
               AsyncBodyEnvelopeTyped, AsyncBodyEnvelope,
               AsyncNetworkKinds, AsyncIngressSources,
               AsyncConfiguration, NoAsyncChunk
    <2> QED BY <2>1
  <1> QED BY <1>1

THEOREM InjectedByzantineNoisePacketIsTyped ==
  \A source \in AsyncIngressSources, recipient \in ValidatorIds,
     nonce \in 0..(AsyncIngressCapacity - 1):
    LET envelope ==
          AsyncBodyEnvelope(recipient, context.height,
                            nodeView[recipient],
                            AsyncHeartbeatSubject, NoAsyncChunk, nonce)
        item == AsyncNetworkItem("Noise", source, envelope)
        packet == PacketForItem(item)
    IN /\ AsyncTypeInvariant
       /\ InjectByzantineNoise(source, recipient, nonce)
       => AsyncPacketTyped(packet)
BY AsyncNoiseItemIsTyped, PacketForTypedItemIsTyped
   DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant

THEOREM InjectByzantineNoisePreservesSchedulerType ==
  \A source \in AsyncIngressSources, recipient \in ValidatorIds,
     nonce \in 0..(AsyncIngressCapacity - 1):
    AsyncTypeInvariant
      /\ InjectByzantineNoise(source, recipient, nonce)
      => AsyncSchedulerTypeInvariant'
PROOF
  <1>1. ASSUME NEW source \in AsyncIngressSources,
                NEW recipient \in ValidatorIds,
                NEW nonce \in 0..(AsyncIngressCapacity - 1),
                AsyncTypeInvariant,
                InjectByzantineNoise(source, recipient, nonce)
         PROVE AsyncSchedulerTypeInvariant'
    <2> DEFINE Envelope ==
          AsyncBodyEnvelope(recipient, context.height,
                            nodeView[recipient],
                            AsyncHeartbeatSubject, NoAsyncChunk, nonce)
    <2> DEFINE Item == AsyncNetworkItem("Noise", source, Envelope)
    <2> DEFINE Packet == PacketForItem(Item)
    <2>1. AsyncPacketTyped(Packet)
      BY <1>1, InjectedByzantineNoisePacketIsTyped
         DEF Envelope, Item, Packet
    <2>2. /\ AsyncTransportContentTypeInvariant
           /\ asyncTransport' = asyncTransport \cup {Packet}
           /\ UNCHANGED AsyncTransportHistoryTypeVars
           /\ UNCHANGED asyncHeldChunks
      BY <1>1, Isa
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncTransportTypeInvariant, InjectByzantineNoise,
             AsyncTransportHistoryTypeVars, Envelope, Item, Packet,
             AsyncCertifiedResponseClaimAuthorityVars,
             LeaveCausalQueues, AsyncSchedulerVars, vars
    <2>3. AsyncTransportContentTypeInvariant'
      BY <2>1, <2>2,
         AddUntrackedTypedPacketPreservesTransportContentType
    <2>4. /\ UNCHANGED AsyncRuntimeScalarTypeVars
           /\ UNCHANGED asyncCausalQueues
           /\ UNCHANGED AsyncIoTopologyTypeVars
           /\ UNCHANGED AsyncIoContentTypeVars
           /\ UNCHANGED AsyncIoCapacityTypeVars
           /\ UNCHANGED AsyncDeferredTopologyTypeVars
           /\ UNCHANGED <<asyncDeferredCompletionQueues,
                          asyncDeferredProgressQueues,
                          asyncDeferredNormalQueues>>
           /\ UNCHANGED AsyncTransportClockTypeVars
           /\ UNCHANGED AsyncIngressTopologyTypeVars
           /\ UNCHANGED asyncIngressLanes
      BY <1>1, Isa
         DEF InjectByzantineNoise, LeaveCausalQueues,
             AsyncRuntimeScalarTypeVars, AsyncIoVars, AsyncDeferredVars,
             AsyncIoTopologyTypeVars, AsyncIoContentTypeVars,
             AsyncIoCapacityTypeVars, AsyncDeferredTopologyTypeVars,
             AsyncTransportClockTypeVars,
             AsyncIngressTopologyTypeVars, AsyncSchedulerVars, vars
    <2>5. AsyncHistoricalRecoveryTypeInvariant'
      BY <1>1, HistoricalRecoveryFramePreservesType, Isa
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             InjectByzantineNoise, AsyncHistoricalRecoveryFrameVars,
             vars
    <2> QED BY <1>1, <2>3, <2>4, <2>5,
                 AsyncTransportContentChangePreservesSchedulerType
         DEF AsyncTypeInvariant
  <1> QED BY <1>1

THEOREM AsyncUntrustedTransportCompletionItemIsTyped ==
  \A kind \in IngressTransportCompletionKinds,
     recipient \in ValidatorIds,
     nonce \in 0..(AsyncIngressCapacity - 1):
    LET item ==
          AsyncUntrustedTransportCompletionItem(kind, recipient, nonce)
    IN /\ TypeInvariant
       /\ AsyncConfiguration
       => AsyncItemTyped(item)
PROOF
  <1>1. ASSUME NEW kind \in IngressTransportCompletionKinds,
                NEW recipient \in ValidatorIds,
                NEW nonce \in 0..(AsyncIngressCapacity - 1)
         PROVE LET item ==
                 AsyncUntrustedTransportCompletionItem(
                   kind, recipient, nonce)
               IN /\ TypeInvariant
                  /\ AsyncConfiguration
                  => AsyncItemTyped(item)
    <2>1. ASSUME TypeInvariant, AsyncConfiguration
           PROVE LET item ==
                   AsyncUntrustedTransportCompletionItem(
                     kind, recipient, nonce)
                 IN AsyncItemTyped(item)
      <3>1. /\ ModelConfiguration
             /\ context.height \in Heights
             /\ nodeView[recipient] \in Views
             /\ AsyncHeartbeatSubject \in ValidSubjects
        BY <1>1, <2>1, AsyncHeartbeatSubjectIsValid
           DEF TypeInvariant
      <3> QED BY <1>1, <2>1, <3>1, SMT
           DEF AsyncUntrustedTransportCompletionItem,
               AsyncUntrustedCertifiedResponseItem,
               AsyncUntrustedCompletionRequestWitness,
               AsyncItemTyped, AsyncNetworkItem,
               AsyncBodyEnvelopeTyped, AsyncBodyEnvelope,
               AsyncCertifiedResponseEnvelope,
               AsyncCertifiedResponseEnvelopeTyped,
               AsyncReplyRequestItemTyped,
               AsyncNetworkKinds, AsyncIngressSources,
               IngressTransportCompletionKinds,
               AsyncConfiguration, NoAsyncChunk
    <2> QED BY <2>1
  <1> QED BY <1>1

THEOREM InjectedUntrustedTransportCompletionPacketIsTyped ==
  \A kind \in IngressTransportCompletionKinds,
     recipient \in ValidatorIds,
     nonce \in 0..(AsyncIngressCapacity - 1):
    LET item ==
          AsyncUntrustedTransportCompletionItem(kind, recipient, nonce)
        packet == PacketForItem(item)
    IN /\ AsyncTypeInvariant
       /\ InjectUntrustedTransportCompletion(kind, recipient, nonce)
       => AsyncPacketTyped(packet)
BY AsyncUntrustedTransportCompletionItemIsTyped,
   PacketForTypedItemIsTyped
   DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant

THEOREM InjectUntrustedTransportCompletionPreservesSchedulerType ==
  \A kind \in IngressTransportCompletionKinds,
     recipient \in ValidatorIds,
     nonce \in 0..(AsyncIngressCapacity - 1):
    AsyncTypeInvariant
      /\ InjectUntrustedTransportCompletion(kind, recipient, nonce)
      => AsyncSchedulerTypeInvariant'
PROOF
  <1>1. ASSUME NEW kind \in IngressTransportCompletionKinds,
                NEW recipient \in ValidatorIds,
                NEW nonce \in 0..(AsyncIngressCapacity - 1),
                AsyncTypeInvariant,
                InjectUntrustedTransportCompletion(
                  kind, recipient, nonce)
         PROVE AsyncSchedulerTypeInvariant'
    <2> DEFINE Item ==
          AsyncUntrustedTransportCompletionItem(kind, recipient, nonce)
    <2> DEFINE Packet == PacketForItem(Item)
    <2>1. AsyncPacketTyped(Packet)
      BY <1>1, InjectedUntrustedTransportCompletionPacketIsTyped
         DEF Item, Packet
    <2>2. /\ AsyncTransportContentTypeInvariant
           /\ asyncTransport' = asyncTransport \cup {Packet}
           /\ UNCHANGED AsyncTransportHistoryTypeVars
           /\ UNCHANGED asyncHeldChunks
      BY <1>1, Isa
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncTransportTypeInvariant,
             InjectUntrustedTransportCompletion,
             AsyncTransportHistoryTypeVars, Item, Packet,
             AsyncCertifiedResponseClaimAuthorityVars,
             LeaveCausalQueues, AsyncSchedulerVars, vars
    <2>3. AsyncTransportContentTypeInvariant'
      BY <2>1, <2>2,
         AddUntrackedTypedPacketPreservesTransportContentType
    <2>4. /\ UNCHANGED AsyncRuntimeScalarTypeVars
           /\ UNCHANGED asyncCausalQueues
           /\ UNCHANGED AsyncIoTopologyTypeVars
           /\ UNCHANGED AsyncIoContentTypeVars
           /\ UNCHANGED AsyncIoCapacityTypeVars
           /\ UNCHANGED AsyncDeferredTopologyTypeVars
           /\ UNCHANGED <<asyncDeferredCompletionQueues,
                          asyncDeferredProgressQueues,
                          asyncDeferredNormalQueues>>
           /\ UNCHANGED AsyncTransportClockTypeVars
           /\ UNCHANGED AsyncIngressTopologyTypeVars
           /\ UNCHANGED asyncIngressLanes
      BY <1>1, Isa
         DEF InjectUntrustedTransportCompletion, LeaveCausalQueues,
             AsyncRuntimeScalarTypeVars, AsyncIoVars, AsyncDeferredVars,
             AsyncIoTopologyTypeVars, AsyncIoContentTypeVars,
             AsyncIoCapacityTypeVars, AsyncDeferredTopologyTypeVars,
             AsyncTransportClockTypeVars,
             AsyncIngressTopologyTypeVars, AsyncSchedulerVars, vars
    <2>5. AsyncHistoricalRecoveryTypeInvariant'
      BY <1>1, HistoricalRecoveryFramePreservesType, Isa
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             InjectUntrustedTransportCompletion,
             AsyncHistoricalRecoveryFrameVars, vars
    <2> QED BY <1>1, <2>3, <2>4, <2>5,
                 AsyncTransportContentChangePreservesSchedulerType
         DEF AsyncTypeInvariant
  <1> QED BY <1>1

THEOREM PublishTypedSingletonPreservesTransportContentType ==
  \A item:
    /\ AsyncTypeInvariant
    /\ AsyncItemTyped(item)
    /\ PublishEphemeralItems({item})
    /\ UNCHANGED
         <<context, AsyncCertifiedResponseClaimCoreAuthorityVars,
           asyncHeldChunks>>
    => AsyncTransportContentTypeInvariant'
PROOF
  <1>1. ASSUME NEW item,
                AsyncTypeInvariant,
                AsyncItemTyped(item),
                PublishEphemeralItems({item}),
                UNCHANGED
                  <<context, AsyncCertifiedResponseClaimCoreAuthorityVars,
                    asyncHeldChunks>>
         PROVE AsyncTransportContentTypeInvariant'
    <2>1. /\ AsyncRuntimeScalarTypeInvariant
           /\ AsyncTransportContentTypeInvariant
      BY <1>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant, AsyncTransportTypeInvariant
    <2>2. /\ IsFiniteSet({item})
           /\ \A queued \in {item}: AsyncItemTyped(queued)
      BY <1>1, FS_Singleton
    <2> QED BY <1>1, <2>1, <2>2,
                 PublishEphemeralItemsPreservesTransportContentType
  <1> QED BY <1>1

THEOREM PublishTypedSingletonPreservesSchedulerType ==
  \A item:
    /\ AsyncTypeInvariant
    /\ AsyncItemTyped(item)
    /\ PublishEphemeralItems({item})
    /\ UNCHANGED
         <<context, AsyncCertifiedResponseClaimCoreAuthorityVars,
           asyncHeldChunks>>
    /\ UNCHANGED AsyncRuntimeScalarTypeVars
    /\ UNCHANGED asyncCausalQueues
    /\ UNCHANGED AsyncIoTopologyTypeVars
    /\ UNCHANGED AsyncIoContentTypeVars
    /\ UNCHANGED AsyncIoCapacityTypeVars
    /\ UNCHANGED AsyncDeferredTopologyTypeVars
    /\ UNCHANGED <<asyncDeferredCompletionQueues,
                    asyncDeferredProgressQueues,
                    asyncDeferredNormalQueues>>
    /\ UNCHANGED AsyncTransportClockTypeVars
    /\ UNCHANGED AsyncIngressTopologyTypeVars
    /\ UNCHANGED asyncIngressLanes
    /\ AsyncHistoricalRecoveryTypeInvariant'
    => AsyncSchedulerTypeInvariant'
BY PublishTypedSingletonPreservesTransportContentType,
   AsyncTransportContentChangePreservesSchedulerType
   DEF AsyncTypeInvariant

THEOREM PublishTypedItemsPreservesSchedulerType ==
  \A items:
    /\ AsyncTypeInvariant
    /\ IsFiniteSet(items)
    /\ \A item \in items: AsyncItemTyped(item)
    /\ PublishEphemeralItems(items)
    /\ UNCHANGED
         <<context, AsyncCertifiedResponseClaimCoreAuthorityVars,
           asyncHeldChunks>>
    /\ UNCHANGED AsyncRuntimeScalarTypeVars
    /\ UNCHANGED asyncCausalQueues
    /\ UNCHANGED AsyncIoTopologyTypeVars
    /\ UNCHANGED AsyncIoContentTypeVars
    /\ UNCHANGED AsyncIoCapacityTypeVars
    /\ UNCHANGED AsyncDeferredTopologyTypeVars
    /\ UNCHANGED <<asyncDeferredCompletionQueues,
                    asyncDeferredProgressQueues,
                    asyncDeferredNormalQueues>>
    /\ UNCHANGED AsyncTransportClockTypeVars
    /\ UNCHANGED AsyncIngressTopologyTypeVars
    /\ UNCHANGED asyncIngressLanes
    /\ AsyncHistoricalRecoveryTypeInvariant'
    => AsyncSchedulerTypeInvariant'
BY PublishEphemeralItemsPreservesTransportContentType,
   AsyncTransportContentChangePreservesSchedulerType
   DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncRuntimeTypeInvariant, AsyncTransportTypeInvariant

THEOREM CurrentVotersAreFinite ==
  TypeInvariant => IsFiniteSet(CurrentVoters)
PROOF
  <1>1. ASSUME TypeInvariant
         PROVE IsFiniteSet(CurrentVoters)
    <2>1. /\ QuorumConfiguration
           /\ CurrentEpoch \in Epochs
      BY <1>1, TypeInvariantMakesCurrentEpochTyped
         DEF TypeInvariant, ModelConfiguration
    <2>2. IsFiniteSet(VotingRoster(CurrentEpoch))
      BY <2>1 DEF QuorumConfiguration
    <2> QED BY <2>2 DEF CurrentVoters
  <1> QED BY <1>1

THEOREM CurrentVotersAreFiniteValidators ==
  TypeInvariant
    => /\ IsFiniteSet(CurrentVoters)
       /\ CurrentVoters \subseteq ValidatorIds
PROOF
  <1>1. ASSUME TypeInvariant
         PROVE /\ IsFiniteSet(CurrentVoters)
               /\ CurrentVoters \subseteq ValidatorIds
    <2>1. IsFiniteSet(CurrentVoters)
      BY <1>1, CurrentVotersAreFinite
    <2>2. /\ QuorumConfiguration
           /\ CurrentEpoch \in Epochs
      BY <1>1, TypeInvariantMakesCurrentEpochTyped
         DEF TypeInvariant, ModelConfiguration
    <2>3. VotingRoster(CurrentEpoch) \subseteq ValidatorIds
      BY <2>2 DEF QuorumConfiguration, VotingRoster
    <2> QED BY <2>1, <2>3 DEF CurrentVoters
  <1> QED BY <1>1

THEOREM ProposalEnvelopeMakesTypedAsyncItem ==
  \A source \in ValidatorIds, envelope \in ProposalEnvelopeSet:
    AsyncItemTyped(AsyncNetworkItem("Proposal", source, envelope))
PROOF
  <1>1. ASSUME NEW source \in ValidatorIds,
                NEW envelope \in ProposalEnvelopeSet
         PROVE AsyncItemTyped(
                 AsyncNetworkItem("Proposal", source, envelope))
    <2>1. /\ DOMAIN AsyncNetworkItem("Proposal", source, envelope) =
                 {"kind", "source", "envelope"}
           /\ AsyncNetworkItem("Proposal", source, envelope).kind =
                "Proposal"
           /\ AsyncNetworkItem("Proposal", source, envelope).source =
                source
           /\ AsyncNetworkItem("Proposal", source, envelope).envelope =
                envelope
      BY DEF AsyncNetworkItem
    <2>2. /\ "Proposal" \in AsyncNetworkKinds
           /\ source \in AsyncIngressSources
           /\ envelope.recipient \in ValidatorIds
      BY <1>1, SMT
         DEF AsyncNetworkKinds, AsyncIngressSources, ProposalEnvelopeSet
    <2> QED BY <1>1, <2>1, <2>2, SMT DEF AsyncItemTyped
  <1> QED BY <1>1

THEOREM VoteEnvelopeMakesTypedAsyncItem ==
  \A source \in ValidatorIds, envelope \in VoteEnvelopeSet:
    AsyncItemTyped(
      AsyncNetworkItem(
        IF envelope.vote.phase = "Prepare"
        THEN "PrepareVote" ELSE "CommitVote",
        source, envelope))
PROOF
  <1>1. ASSUME NEW source \in ValidatorIds,
                NEW envelope \in VoteEnvelopeSet
         PROVE AsyncItemTyped(
                 AsyncNetworkItem(
                   IF envelope.vote.phase = "Prepare"
                   THEN "PrepareVote" ELSE "CommitVote",
                   source, envelope))
    <2>1. /\ envelope.recipient \in ValidatorIds
           /\ envelope.vote.phase \in Phases
           /\ source \in AsyncIngressSources
      BY <1>1, SMT
         DEF VoteEnvelopeSet, VoteRecordSet, AsyncIngressSources
    <2>2. CASE envelope.vote.phase = "Prepare"
      BY <1>1, <2>1, <2>2, SMT
         DEF AsyncItemTyped, AsyncNetworkItem, AsyncNetworkKinds
    <2>3. CASE envelope.vote.phase # "Prepare"
      BY <1>1, <2>1, <2>3, SMT
         DEF AsyncItemTyped, AsyncNetworkItem,
             AsyncNetworkKinds, Phases
    <2> QED BY <2>2, <2>3
  <1> QED BY <1>1

THEOREM TimeoutEnvelopeMakesTypedAsyncItem ==
  \A source \in ValidatorIds, envelope \in TimeoutEnvelopeSet:
    AsyncItemTyped(AsyncNetworkItem("TimeoutVote", source, envelope))
PROOF
  <1>1. ASSUME NEW source \in ValidatorIds,
                NEW envelope \in TimeoutEnvelopeSet
         PROVE AsyncItemTyped(
                 AsyncNetworkItem("TimeoutVote", source, envelope))
    <2>1. /\ DOMAIN AsyncNetworkItem("TimeoutVote", source, envelope) =
                 {"kind", "source", "envelope"}
           /\ AsyncNetworkItem("TimeoutVote", source, envelope).kind =
                "TimeoutVote"
           /\ AsyncNetworkItem("TimeoutVote", source, envelope).source =
                source
           /\ AsyncNetworkItem("TimeoutVote", source, envelope).envelope =
                envelope
      BY DEF AsyncNetworkItem
    <2>2. /\ "TimeoutVote" \in AsyncNetworkKinds
           /\ source \in AsyncIngressSources
           /\ envelope.recipient \in ValidatorIds
      BY <1>1, SMT
         DEF AsyncNetworkKinds, AsyncIngressSources, TimeoutEnvelopeSet
    <2> QED BY <1>1, <2>1, <2>2, SMT DEF AsyncItemTyped
  <1> QED BY <1>1

THEOREM ByzantineProposalItemIsTyped ==
  \A signer \in ValidatorIds, recipient \in ValidatorIds,
     roundView \in Views, subject \in Subjects,
     timeoutCertificate \in TimeoutCertificateOptionSet,
     highestPrepare \in PrepareQcOptionSet:
    TypeInvariant
      => AsyncItemTyped(
           AsyncNetworkItem(
             "Proposal", signer,
             ProposalEnvelope(
               recipient,
               Proposal(context, roundView, subject, signer,
                        timeoutCertificate, highestPrepare))))
PROOF
  <1>1. ASSUME NEW signer \in ValidatorIds,
                NEW recipient \in ValidatorIds,
                NEW roundView \in Views,
                NEW subject \in Subjects,
                NEW timeoutCertificate \in TimeoutCertificateOptionSet,
                NEW highestPrepare \in PrepareQcOptionSet,
                TypeInvariant
         PROVE AsyncItemTyped(
                 AsyncNetworkItem(
                   "Proposal", signer,
                   ProposalEnvelope(
                     recipient,
                     Proposal(context, roundView, subject, signer,
                        timeoutCertificate, highestPrepare))))
    <2>1. /\ context \in ContextRecords
           /\ context.height \in Heights
      BY <1>1 DEF TypeInvariant
    <2>2. Proposal(context, roundView, subject, signer,
                        timeoutCertificate, highestPrepare) \in ProposalRecordSet
      BY <1>1, <2>1, SMT DEF Proposal, ProposalRecordSet
    <2>3. ProposalEnvelope(
             recipient,
             Proposal(context, roundView, subject, signer,
                        timeoutCertificate, highestPrepare)) \in ProposalEnvelopeSet
      BY <1>1, <2>2, SMT DEF ProposalEnvelope, ProposalEnvelopeSet
    <2> QED BY <1>1, <2>3,
                 ProposalEnvelopeMakesTypedAsyncItem
  <1> QED BY <1>1

THEOREM ByzantineProposalOutboxIsFiniteAndTyped ==
  \A signer \in ValidatorIds, roundView \in Views,
     subject \in Subjects,
     timeoutCertificate \in TimeoutCertificateOptionSet,
     highestPrepare \in PrepareQcOptionSet:
    LET proposal == Proposal(context, roundView, subject, signer,
                        timeoutCertificate, highestPrepare)
        items == ByzantineProposalOutbox(signer, proposal)
    IN TypeInvariant
         => /\ IsFiniteSet(items)
            /\ \A item \in items: AsyncItemTyped(item)
PROOF
  <1>1. ASSUME NEW signer \in ValidatorIds,
                NEW roundView \in Views,
                NEW subject \in Subjects,
                NEW timeoutCertificate \in TimeoutCertificateOptionSet,
                NEW highestPrepare \in PrepareQcOptionSet
         PROVE LET proposal ==
                 Proposal(context, roundView, subject, signer,
                        timeoutCertificate, highestPrepare)
               items == ByzantineProposalOutbox(signer, proposal)
               IN TypeInvariant
                    => /\ IsFiniteSet(items)
                       /\ \A item \in items: AsyncItemTyped(item)
    <2>1. ASSUME TypeInvariant
           PROVE LET proposal ==
                   Proposal(context, roundView, subject, signer,
                        timeoutCertificate, highestPrepare)
                 items == ByzantineProposalOutbox(signer, proposal)
                 IN /\ IsFiniteSet(items)
                    /\ \A item \in items: AsyncItemTyped(item)
      <3>1. /\ IsFiniteSet(CurrentVoters)
             /\ CurrentVoters \subseteq ValidatorIds
             /\ context \in ContextRecords
             /\ context.height \in Heights
        BY <2>1, CurrentVotersAreFiniteValidators
           DEF TypeInvariant
      <3>2. IsFiniteSet(
               ByzantineProposalOutbox(
                 signer, Proposal(context, roundView, subject, signer,
                        timeoutCertificate, highestPrepare)))
        BY <3>1, FS_Image DEF ByzantineProposalOutbox
      <3>3. \A item \in
                    ByzantineProposalOutbox(
                      signer,
                      Proposal(context, roundView, subject, signer,
                        timeoutCertificate, highestPrepare)):
               AsyncItemTyped(item)
        <4>1. ASSUME NEW item \in
                     ByzantineProposalOutbox(
                       signer,
                       Proposal(context, roundView, subject, signer,
                        timeoutCertificate, highestPrepare))
               PROVE AsyncItemTyped(item)
          <5>1. PICK recipient \in CurrentVoters:
                   item = AsyncNetworkItem(
                     "Proposal", signer,
                     ProposalEnvelope(
                       recipient,
                       Proposal(context, roundView, subject, signer,
                        timeoutCertificate, highestPrepare)))
            BY <4>1 DEF ByzantineProposalOutbox
          <5>2. recipient \in ValidatorIds
            BY <3>1, <5>1
          <5>3. AsyncItemTyped(
                   AsyncNetworkItem(
                     "Proposal", signer,
                     ProposalEnvelope(
                       recipient,
                       Proposal(context, roundView, subject, signer,
                        timeoutCertificate, highestPrepare))))
            BY <1>1, <2>1, <5>2, ByzantineProposalItemIsTyped
          <5> QED BY <5>1, <5>3
        <4> QED BY <4>1
      <3> QED BY <3>2, <3>3
    <2> QED BY <2>1
  <1> QED BY <1>1

THEOREM AsyncByzantineProposalPreservesSchedulerType ==
  \A signer \in ValidatorIds, roundView \in Views,
     subject \in Subjects,
     timeoutCertificate \in TimeoutCertificateOptionSet,
     highestPrepare \in PrepareQcOptionSet:
    AsyncTypeInvariant
      /\ AsyncByzantineProposal(
                    signer, roundView, subject,
                    timeoutCertificate, highestPrepare)
      => AsyncSchedulerTypeInvariant'
PROOF
  <1>1. ASSUME NEW signer \in ValidatorIds,
                NEW roundView \in Views,
                NEW subject \in Subjects,
                NEW timeoutCertificate \in TimeoutCertificateOptionSet,
                NEW highestPrepare \in PrepareQcOptionSet,
                AsyncTypeInvariant,
                AsyncByzantineProposal(
                    signer, roundView, subject,
                    timeoutCertificate, highestPrepare)
         PROVE AsyncSchedulerTypeInvariant'
    <2> DEFINE ProposalValue ==
          Proposal(context, roundView, subject, signer,
                        timeoutCertificate, highestPrepare)
    <2> DEFINE Items ==
          ByzantineProposalOutbox(signer, ProposalValue)
    <2>1. /\ IsFiniteSet(Items)
           /\ \A item \in Items: AsyncItemTyped(item)
      BY <1>1, ByzantineProposalOutboxIsFiniteAndTyped
         DEF AsyncTypeInvariant, ProposalValue, Items
    <2>2. /\ PublishEphemeralItems(Items)
           /\ UNCHANGED <<context, asyncHeldChunks>>
           /\ UNCHANGED AsyncRuntimeScalarTypeVars
           /\ UNCHANGED asyncCausalQueues
           /\ UNCHANGED AsyncIoTopologyTypeVars
           /\ UNCHANGED AsyncIoContentTypeVars
           /\ UNCHANGED AsyncIoCapacityTypeVars
           /\ UNCHANGED AsyncDeferredTopologyTypeVars
           /\ UNCHANGED <<asyncDeferredCompletionQueues,
                          asyncDeferredProgressQueues,
                          asyncDeferredNormalQueues>>
           /\ UNCHANGED AsyncTransportClockTypeVars
           /\ UNCHANGED AsyncIngressTopologyTypeVars
           /\ UNCHANGED asyncIngressLanes
      BY <1>1, Isa
         DEF AsyncByzantineProposal, ByzantineBroadcastProposal,
             ProposalValue, Items, AsyncRuntimeScalarTypeVars,
             AsyncIoVars, AsyncDeferredVars,
             AsyncIoTopologyTypeVars, AsyncIoContentTypeVars,
             AsyncIoCapacityTypeVars, AsyncDeferredTopologyTypeVars,
             AsyncTransportClockTypeVars,
             AsyncIngressTopologyTypeVars, AsyncSchedulerVars, vars
    <2>3. AsyncHistoricalRecoveryTypeInvariant'
      BY <1>1, HistoricalRecoveryFramePreservesType, Isa
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncByzantineProposal, ByzantineBroadcastProposal,
             AsyncHistoricalRecoveryFrameVars, vars
    <2> QED BY <1>1, <2>1, <2>2, <2>3,
                 PublishTypedItemsPreservesSchedulerType
  <1> QED BY <1>1

THEOREM ByzantineVoteItemIsTyped ==
  \A signer \in ValidatorIds, recipient \in ValidatorIds,
     roundView \in Views, phase \in Phases, subject \in Subjects:
    TypeInvariant
      => AsyncItemTyped(
           AsyncNetworkItem(
             IF phase = "Prepare" THEN "PrepareVote" ELSE "CommitVote",
             signer,
             VoteEnvelope(
               recipient,
               Vote(context, roundView, phase, subject, signer))))
PROOF
  <1>1. ASSUME NEW signer \in ValidatorIds,
                NEW recipient \in ValidatorIds,
                NEW roundView \in Views,
                NEW phase \in Phases,
                NEW subject \in Subjects,
                TypeInvariant
         PROVE AsyncItemTyped(
                 AsyncNetworkItem(
                   IF phase = "Prepare"
                   THEN "PrepareVote" ELSE "CommitVote",
                   signer,
                   VoteEnvelope(
                     recipient,
                     Vote(context, roundView, phase, subject, signer))))
    <2>1. /\ context \in ContextRecords
           /\ context.height \in Heights
      BY <1>1 DEF TypeInvariant
    <2>2. Vote(context, roundView, phase, subject, signer)
             \in VoteRecordSet
      BY <1>1, <2>1, SMT DEF Vote, VoteRecordSet
    <2>3. VoteEnvelope(
             recipient,
             Vote(context, roundView, phase, subject, signer))
             \in VoteEnvelopeSet
      BY <1>1, <2>2, SMT DEF VoteEnvelope, VoteEnvelopeSet
    <2>4. VoteEnvelope(
             recipient,
             Vote(context, roundView, phase, subject, signer)).vote.phase
             = phase
      BY DEF VoteEnvelope, Vote
    <2> QED BY <1>1, <2>3, <2>4,
                 VoteEnvelopeMakesTypedAsyncItem
  <1> QED BY <1>1

THEOREM ByzantineVoteOutboxIsFiniteAndTyped ==
  \A signer \in ValidatorIds, roundView \in Views,
     phase \in Phases, subject \in Subjects:
    LET vote == Vote(context, roundView, phase, subject, signer)
        items == ByzantineVoteOutbox(signer, vote)
    IN TypeInvariant
         => /\ IsFiniteSet(items)
            /\ \A item \in items: AsyncItemTyped(item)
PROOF
  <1>1. ASSUME NEW signer \in ValidatorIds,
                NEW roundView \in Views,
                NEW phase \in Phases,
                NEW subject \in Subjects
         PROVE LET vote ==
                 Vote(context, roundView, phase, subject, signer)
               items == ByzantineVoteOutbox(signer, vote)
               IN TypeInvariant
                    => /\ IsFiniteSet(items)
                       /\ \A item \in items: AsyncItemTyped(item)
    <2>1. ASSUME TypeInvariant
           PROVE LET vote ==
                   Vote(context, roundView, phase, subject, signer)
                 items == ByzantineVoteOutbox(signer, vote)
                 IN /\ IsFiniteSet(items)
                    /\ \A item \in items: AsyncItemTyped(item)
      <3>1. /\ IsFiniteSet(CurrentVoters)
             /\ CurrentVoters \subseteq ValidatorIds
             /\ context \in ContextRecords
             /\ context.height \in Heights
        BY <2>1, CurrentVotersAreFiniteValidators
           DEF TypeInvariant
      <3>2. IsFiniteSet(
               ByzantineVoteOutbox(
                 signer, Vote(context, roundView, phase, subject, signer)))
        BY <3>1, FS_Image DEF ByzantineVoteOutbox
      <3>3. \A item \in
                    ByzantineVoteOutbox(
                      signer,
                      Vote(context, roundView, phase, subject, signer)):
               AsyncItemTyped(item)
        <4>1. ASSUME NEW item \in
                     ByzantineVoteOutbox(
                       signer,
                       Vote(context, roundView, phase, subject, signer))
               PROVE AsyncItemTyped(item)
          <5>1. PICK recipient \in CurrentVoters:
                   item = AsyncNetworkItem(
                     IF phase = "Prepare" THEN "PrepareVote"
                     ELSE "CommitVote",
                     signer,
                     VoteEnvelope(
                       recipient,
                       Vote(context, roundView, phase, subject, signer)))
            BY <4>1 DEF ByzantineVoteOutbox
          <5>2. recipient \in ValidatorIds
            BY <3>1, <5>1
          <5>3. AsyncItemTyped(
                   AsyncNetworkItem(
                     IF phase = "Prepare"
                     THEN "PrepareVote" ELSE "CommitVote",
                     signer,
                     VoteEnvelope(
                       recipient,
                       Vote(context, roundView, phase, subject, signer))))
            BY <1>1, <2>1, <5>2, ByzantineVoteItemIsTyped
          <5> QED BY <5>1, <5>3
        <4> QED BY <4>1
      <3> QED BY <3>2, <3>3
    <2> QED BY <2>1
  <1> QED BY <1>1

THEOREM AsyncByzantineVotePreservesSchedulerType ==
  \A signer \in ValidatorIds, roundView \in Views,
     phase \in Phases, subject \in Subjects:
    AsyncTypeInvariant
      /\ AsyncByzantineVote(signer, roundView, phase, subject)
      => AsyncSchedulerTypeInvariant'
PROOF
  <1>1. ASSUME NEW signer \in ValidatorIds,
                NEW roundView \in Views,
                NEW phase \in Phases,
                NEW subject \in Subjects,
                AsyncTypeInvariant,
                AsyncByzantineVote(signer, roundView, phase, subject)
         PROVE AsyncSchedulerTypeInvariant'
    <2> DEFINE VoteValue ==
          Vote(context, roundView, phase, subject, signer)
    <2> DEFINE Items == ByzantineVoteOutbox(signer, VoteValue)
    <2>1. /\ IsFiniteSet(Items)
           /\ \A item \in Items: AsyncItemTyped(item)
      BY <1>1, ByzantineVoteOutboxIsFiniteAndTyped
         DEF AsyncTypeInvariant, VoteValue, Items
    <2>2. /\ PublishEphemeralItems(Items)
           /\ UNCHANGED <<context, asyncHeldChunks>>
           /\ UNCHANGED AsyncRuntimeScalarTypeVars
           /\ UNCHANGED asyncCausalQueues
           /\ UNCHANGED AsyncIoTopologyTypeVars
           /\ UNCHANGED AsyncIoContentTypeVars
           /\ UNCHANGED AsyncIoCapacityTypeVars
           /\ UNCHANGED AsyncDeferredTopologyTypeVars
           /\ UNCHANGED <<asyncDeferredCompletionQueues,
                          asyncDeferredProgressQueues,
                          asyncDeferredNormalQueues>>
           /\ UNCHANGED AsyncTransportClockTypeVars
           /\ UNCHANGED AsyncIngressTopologyTypeVars
           /\ UNCHANGED asyncIngressLanes
      BY <1>1, Isa
         DEF AsyncByzantineVote, ByzantineBroadcastVote,
             VoteValue, Items, AsyncRuntimeScalarTypeVars,
             AsyncIoVars, AsyncDeferredVars,
             AsyncIoTopologyTypeVars, AsyncIoContentTypeVars,
             AsyncIoCapacityTypeVars, AsyncDeferredTopologyTypeVars,
             AsyncTransportClockTypeVars,
             AsyncIngressTopologyTypeVars, AsyncSchedulerVars, vars
    <2>3. AsyncHistoricalRecoveryTypeInvariant'
      BY <1>1, HistoricalRecoveryFramePreservesType, Isa
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncByzantineVote, ByzantineBroadcastVote,
             AsyncHistoricalRecoveryFrameVars, vars
    <2> QED BY <1>1, <2>1, <2>2, <2>3,
                 PublishTypedItemsPreservesSchedulerType
  <1> QED BY <1>1

THEOREM ByzantineTimeoutItemIsTyped ==
  \A signer \in ValidatorIds, recipient \in ValidatorIds,
     roundView \in Views, highestPrepare \in PrepareQcOptionSet:
    TypeInvariant
      => AsyncItemTyped(
           AsyncNetworkItem(
             "TimeoutVote", signer,
             TimeoutEnvelope(
               recipient,
               TimeoutVote(context, roundView, signer, highestPrepare))))
PROOF
  <1>1. ASSUME NEW signer \in ValidatorIds,
                NEW recipient \in ValidatorIds,
                NEW roundView \in Views,
                NEW highestPrepare \in PrepareQcOptionSet,
                TypeInvariant
         PROVE AsyncItemTyped(
                 AsyncNetworkItem(
                   "TimeoutVote", signer,
                   TimeoutEnvelope(
                     recipient,
                     TimeoutVote(context, roundView, signer, highestPrepare))))
    <2>1. /\ context \in ContextRecords
           /\ context.height \in Heights
      BY <1>1 DEF TypeInvariant
    <2>2. TimeoutVote(context, roundView, signer, highestPrepare) \in TimeoutVoteRecordSet
      BY <1>1, <2>1, SMT
         DEF TimeoutVote, TimeoutVoteRecordSet
    <2>3. TimeoutEnvelope(
             recipient,
             TimeoutVote(context, roundView, signer, highestPrepare)) \in TimeoutEnvelopeSet
      BY <1>1, <2>2, SMT DEF TimeoutEnvelope, TimeoutEnvelopeSet
    <2> QED BY <1>1, <2>3,
                 TimeoutEnvelopeMakesTypedAsyncItem
  <1> QED BY <1>1

THEOREM ByzantineTimeoutOutboxIsFiniteAndTyped ==
  \A signer \in ValidatorIds, roundView \in Views,
     highestPrepare \in PrepareQcOptionSet:
    LET vote ==
          TimeoutVote(context, roundView, signer, highestPrepare)
        items == ByzantineTimeoutOutbox(signer, vote)
    IN TypeInvariant
         => /\ IsFiniteSet(items)
            /\ \A item \in items: AsyncItemTyped(item)
PROOF
  <1>1. ASSUME NEW signer \in ValidatorIds,
                NEW roundView \in Views,
                NEW highestPrepare \in PrepareQcOptionSet
         PROVE LET vote ==
                 TimeoutVote(context, roundView, signer, highestPrepare)
               items == ByzantineTimeoutOutbox(signer, vote)
               IN TypeInvariant
                    => /\ IsFiniteSet(items)
                       /\ \A item \in items: AsyncItemTyped(item)
    <2>1. ASSUME TypeInvariant
           PROVE LET vote ==
                   TimeoutVote(context, roundView, signer, highestPrepare)
                 items == ByzantineTimeoutOutbox(signer, vote)
                 IN /\ IsFiniteSet(items)
                    /\ \A item \in items: AsyncItemTyped(item)
      <3>1. /\ IsFiniteSet(CurrentVoters)
             /\ CurrentVoters \subseteq ValidatorIds
             /\ context \in ContextRecords
             /\ context.height \in Heights
        BY <2>1, CurrentVotersAreFiniteValidators
           DEF TypeInvariant
      <3>2. IsFiniteSet(
               ByzantineTimeoutOutbox(
                 signer,
                 TimeoutVote(context, roundView, signer, highestPrepare)))
        BY <3>1, FS_Image DEF ByzantineTimeoutOutbox
      <3>3. \A item \in
                    ByzantineTimeoutOutbox(
                      signer,
                      TimeoutVote(context, roundView, signer, highestPrepare)):
               AsyncItemTyped(item)
        <4>1. ASSUME NEW item \in
                     ByzantineTimeoutOutbox(
                       signer,
                       TimeoutVote(context, roundView, signer, highestPrepare))
               PROVE AsyncItemTyped(item)
          <5>1. PICK recipient \in CurrentVoters:
                   item = AsyncNetworkItem(
                     "TimeoutVote", signer,
                     TimeoutEnvelope(
                       recipient,
                       TimeoutVote(context, roundView, signer, highestPrepare)))
            BY <4>1 DEF ByzantineTimeoutOutbox
          <5>2. recipient \in ValidatorIds
            BY <3>1, <5>1
          <5>3. AsyncItemTyped(
                   AsyncNetworkItem(
                     "TimeoutVote", signer,
                     TimeoutEnvelope(
                       recipient,
                       TimeoutVote(context, roundView, signer, highestPrepare))))
            BY <1>1, <2>1, <5>2, ByzantineTimeoutItemIsTyped
          <5> QED BY <5>1, <5>3
        <4> QED BY <4>1
      <3> QED BY <3>2, <3>3
    <2> QED BY <2>1
  <1> QED BY <1>1

THEOREM AsyncByzantineTimeoutPreservesSchedulerType ==
  \A signer \in ValidatorIds, roundView \in Views,
     highestPrepare \in PrepareQcOptionSet:
    AsyncTypeInvariant
      /\ AsyncByzantineTimeout(
                    signer, roundView, highestPrepare)
      => AsyncSchedulerTypeInvariant'
PROOF
  <1>1. ASSUME NEW signer \in ValidatorIds,
                NEW roundView \in Views,
                NEW highestPrepare \in PrepareQcOptionSet,
                AsyncTypeInvariant,
                AsyncByzantineTimeout(
                    signer, roundView, highestPrepare)
         PROVE AsyncSchedulerTypeInvariant'
    <2> DEFINE VoteValue ==
          TimeoutVote(context, roundView, signer, highestPrepare)
    <2> DEFINE Items == ByzantineTimeoutOutbox(signer, VoteValue)
    <2>1. /\ IsFiniteSet(Items)
           /\ \A item \in Items: AsyncItemTyped(item)
      BY <1>1, ByzantineTimeoutOutboxIsFiniteAndTyped
         DEF AsyncTypeInvariant, VoteValue, Items
    <2>2. /\ PublishEphemeralItems(Items)
           /\ UNCHANGED <<context, asyncHeldChunks>>
           /\ UNCHANGED AsyncRuntimeScalarTypeVars
           /\ UNCHANGED asyncCausalQueues
           /\ UNCHANGED AsyncIoTopologyTypeVars
           /\ UNCHANGED AsyncIoContentTypeVars
           /\ UNCHANGED AsyncIoCapacityTypeVars
           /\ UNCHANGED AsyncDeferredTopologyTypeVars
           /\ UNCHANGED <<asyncDeferredCompletionQueues,
                          asyncDeferredProgressQueues,
                          asyncDeferredNormalQueues>>
           /\ UNCHANGED AsyncTransportClockTypeVars
           /\ UNCHANGED AsyncIngressTopologyTypeVars
           /\ UNCHANGED asyncIngressLanes
      BY <1>1, Isa
         DEF AsyncByzantineTimeout, ByzantineBroadcastTimeout,
             VoteValue, Items, AsyncRuntimeScalarTypeVars,
             AsyncIoVars, AsyncDeferredVars,
             AsyncIoTopologyTypeVars, AsyncIoContentTypeVars,
             AsyncIoCapacityTypeVars, AsyncDeferredTopologyTypeVars,
             AsyncTransportClockTypeVars,
             AsyncIngressTopologyTypeVars, AsyncSchedulerVars, vars
    <2>3. AsyncHistoricalRecoveryTypeInvariant'
      BY <1>1, HistoricalRecoveryFramePreservesType, Isa
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncByzantineTimeout, ByzantineBroadcastTimeout,
             AsyncHistoricalRecoveryFrameVars, vars
    <2> QED BY <1>1, <2>1, <2>2, <2>3,
                 PublishTypedItemsPreservesSchedulerType
  <1> QED BY <1>1

THEOREM AuthenticatedJunkItemIsTyped ==
  \A kind \in {"NormalJunk", "ProgressJunk"},
     source \in ValidatorIds, recipient \in ValidatorIds,
     nonce \in 0..(AsyncIngressCapacity - 1):
    LET envelope ==
          AsyncBodyEnvelope(recipient, context.height,
                            nodeView[recipient],
                            AsyncHeartbeatSubject, NoAsyncChunk, nonce)
        item == AsyncNetworkItem(kind, source, envelope)
    IN /\ TypeInvariant
       /\ AsyncConfiguration
       => AsyncItemTyped(item)
PROOF
  <1>1. ASSUME NEW kind \in {"NormalJunk", "ProgressJunk"},
                NEW source \in ValidatorIds,
                NEW recipient \in ValidatorIds,
                NEW nonce \in 0..(AsyncIngressCapacity - 1)
         PROVE LET envelope ==
                 AsyncBodyEnvelope(recipient, context.height,
                                   nodeView[recipient],
                                   AsyncHeartbeatSubject,
                                   NoAsyncChunk, nonce)
               item == AsyncNetworkItem(kind, source, envelope)
               IN /\ TypeInvariant
                  /\ AsyncConfiguration
                  => AsyncItemTyped(item)
    <2>1. ASSUME TypeInvariant, AsyncConfiguration
           PROVE LET envelope ==
                   AsyncBodyEnvelope(recipient, context.height,
                                     nodeView[recipient],
                                     AsyncHeartbeatSubject,
                                     NoAsyncChunk, nonce)
                 item == AsyncNetworkItem(kind, source, envelope)
                 IN AsyncItemTyped(item)
      <3>1. /\ ModelConfiguration
             /\ context.height \in Heights
             /\ nodeView[recipient] \in Views
             /\ AsyncHeartbeatSubject \in ValidSubjects
        BY <1>1, <2>1, AsyncHeartbeatSubjectIsValid
           DEF TypeInvariant
      <3> QED BY <1>1, <2>1, <3>1, SMT
           DEF AsyncItemTyped, AsyncNetworkItem,
               AsyncBodyEnvelopeTyped, AsyncBodyEnvelope,
               AsyncNetworkKinds, AsyncIngressSources,
               AsyncConfiguration, NoAsyncChunk
    <2> QED BY <2>1
  <1> QED BY <1>1

THEOREM InjectAuthenticatedJunkPreservesSchedulerType ==
  \A kind \in {"NormalJunk", "ProgressJunk"},
     source \in ValidatorIds, recipient \in ValidatorIds,
     nonce \in 0..(AsyncIngressCapacity - 1):
    AsyncTypeInvariant
      /\ InjectAuthenticatedJunk(kind, source, recipient, nonce)
      => AsyncSchedulerTypeInvariant'
PROOF
  <1>1. ASSUME NEW kind \in {"NormalJunk", "ProgressJunk"},
                NEW source \in ValidatorIds,
                NEW recipient \in ValidatorIds,
                NEW nonce \in 0..(AsyncIngressCapacity - 1),
                AsyncTypeInvariant,
                InjectAuthenticatedJunk(kind, source, recipient, nonce)
         PROVE AsyncSchedulerTypeInvariant'
    <2> DEFINE Envelope ==
          AsyncBodyEnvelope(recipient, context.height,
                            nodeView[recipient],
                            AsyncHeartbeatSubject, NoAsyncChunk, nonce)
    <2> DEFINE Item == AsyncNetworkItem(kind, source, Envelope)
    <2>1. AsyncItemTyped(Item)
      BY <1>1, AuthenticatedJunkItemIsTyped
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
             Envelope, Item
    <2>2. PublishEphemeralItems({Item})
      BY <1>1, Isa
         DEF InjectAuthenticatedJunk, PublishEphemeralItems,
             PacketsForItems, Envelope, Item
    <2>3. /\ UNCHANGED <<context, asyncHeldChunks>>
           /\ UNCHANGED AsyncRuntimeScalarTypeVars
           /\ UNCHANGED asyncCausalQueues
           /\ UNCHANGED AsyncIoTopologyTypeVars
           /\ UNCHANGED AsyncIoContentTypeVars
           /\ UNCHANGED AsyncIoCapacityTypeVars
           /\ UNCHANGED AsyncDeferredTopologyTypeVars
           /\ UNCHANGED <<asyncDeferredCompletionQueues,
                          asyncDeferredProgressQueues,
                          asyncDeferredNormalQueues>>
           /\ UNCHANGED AsyncTransportClockTypeVars
           /\ UNCHANGED AsyncIngressTopologyTypeVars
           /\ UNCHANGED asyncIngressLanes
      BY <1>1, Isa
         DEF InjectAuthenticatedJunk, LeaveCausalQueues,
             AsyncRuntimeScalarTypeVars, AsyncIoVars, AsyncDeferredVars,
             AsyncIoTopologyTypeVars, AsyncIoContentTypeVars,
             AsyncIoCapacityTypeVars, AsyncDeferredTopologyTypeVars,
             AsyncTransportClockTypeVars,
             AsyncIngressTopologyTypeVars, AsyncSchedulerVars, vars
    <2>4. AsyncHistoricalRecoveryTypeInvariant'
      BY <1>1, HistoricalRecoveryFramePreservesType, Isa
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             InjectAuthenticatedJunk, AsyncHistoricalRecoveryFrameVars,
             vars
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4,
                 PublishTypedSingletonPreservesSchedulerType
  <1> QED BY <1>1

THEOREM CertifiedRequestEnvelopeMakesTypedAsyncItem ==
  \A source \in ValidatorIds, recipient \in ValidatorIds,
     qc \in QcRecordSet,
     nonce \in 0..(AsyncIngressCapacity - 1):
    AsyncConfiguration
      => AsyncItemTyped(
           AsyncNetworkItem(
             "CertifiedRequest", source,
             AsyncCertifiedRequestEnvelope(
               recipient, source, qc, nonce)))
BY SMTT(60)
   DEF AsyncItemTyped, AsyncReplyRequestItemTyped,
       AsyncCertifiedRequestEnvelope, AsyncNetworkItem,
       AsyncNetworkKinds, AsyncIngressSources, AsyncConfiguration

THEOREM CertifiedRequestFieldsMakeTypedItem ==
  \A source \in ValidatorIds, recipient \in ValidatorIds,
     qc \in QcRecordSet,
     nonce \in 0..(AsyncIngressCapacity - 1):
    AsyncConfiguration
      => AsyncItemTyped(
           AsyncNetworkItem(
             "CertifiedRequest", source,
             AsyncCertifiedRequestEnvelope(
               recipient, source, qc, nonce)))
BY CertifiedRequestEnvelopeMakesTypedAsyncItem

THEOREM ByzantineCertifiedRequestItemIsTyped ==
  \A source \in ValidatorIds, recipient \in ValidatorIds,
     qc \in commitQCs, nonce \in 0..(AsyncIngressCapacity - 1):
    LET envelope ==
          AsyncCertifiedRequestEnvelope(recipient, source, qc, nonce)
        item == AsyncNetworkItem("CertifiedRequest", source, envelope)
    IN /\ StrongInductiveInvariant
       /\ AsyncConfiguration
       => AsyncItemTyped(item)
PROOF
  <1>1. ASSUME NEW source \in ValidatorIds,
                NEW recipient \in ValidatorIds,
                NEW qc \in commitQCs,
                NEW nonce \in 0..(AsyncIngressCapacity - 1)
         PROVE LET envelope ==
                 AsyncCertifiedRequestEnvelope(
                   recipient, source, qc, nonce)
               item ==
                 AsyncNetworkItem("CertifiedRequest", source, envelope)
               IN /\ StrongInductiveInvariant
                  /\ AsyncConfiguration
                  => AsyncItemTyped(item)
    <2>1. ASSUME StrongInductiveInvariant, AsyncConfiguration
           PROVE LET envelope ==
                   AsyncCertifiedRequestEnvelope(
                     recipient, source, qc, nonce)
                 item ==
                   AsyncNetworkItem("CertifiedRequest", source, envelope)
                 IN AsyncItemTyped(item)
      <3>1. /\ context.height \in Heights
             /\ qc \in QcRecordSet
        BY <1>1, <2>1
           DEF StrongInductiveInvariant, Safety, TypeInvariant,
               ReducerProvenanceInvariant, CertificatesBackedByIntents,
               HistoricalQcValid
      <3> QED BY <1>1, <2>1, <3>1,
                   CertifiedRequestFieldsMakeTypedItem
    <2> QED BY <2>1
  <1> QED BY <1>1

THEOREM InjectByzantineCertifiedRequestPreservesSchedulerType ==
  \A source \in ValidatorIds, recipient \in ValidatorIds,
     qc \in commitQCs, nonce \in 0..(AsyncIngressCapacity - 1):
    /\ StrongInductiveInvariant
    /\ AsyncTypeInvariant
    /\ InjectByzantineCertifiedRequest(source, recipient, qc, nonce)
    => AsyncSchedulerTypeInvariant'
PROOF
  <1>1. ASSUME NEW source \in ValidatorIds,
                NEW recipient \in ValidatorIds,
                NEW qc \in commitQCs,
                NEW nonce \in 0..(AsyncIngressCapacity - 1),
                StrongInductiveInvariant,
                AsyncTypeInvariant,
                InjectByzantineCertifiedRequest(
                  source, recipient, qc, nonce)
         PROVE AsyncSchedulerTypeInvariant'
    <2> DEFINE Envelope ==
          AsyncCertifiedRequestEnvelope(recipient, source, qc, nonce)
    <2> DEFINE Item ==
          AsyncNetworkItem("CertifiedRequest", source, Envelope)
    <2>1. AsyncItemTyped(Item)
      BY <1>1, ByzantineCertifiedRequestItemIsTyped
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
             Envelope, Item
    <2>2. PublishEphemeralItems({Item})
      BY <1>1, Isa
         DEF InjectByzantineCertifiedRequest, PublishEphemeralItems,
             PacketsForItems, Envelope, Item
    <2>3. /\ UNCHANGED <<context, asyncHeldChunks>>
           /\ UNCHANGED AsyncRuntimeScalarTypeVars
           /\ UNCHANGED asyncCausalQueues
           /\ UNCHANGED AsyncIoTopologyTypeVars
           /\ UNCHANGED AsyncIoContentTypeVars
           /\ UNCHANGED AsyncIoCapacityTypeVars
           /\ UNCHANGED AsyncDeferredTopologyTypeVars
           /\ UNCHANGED <<asyncDeferredCompletionQueues,
                          asyncDeferredProgressQueues,
                          asyncDeferredNormalQueues>>
           /\ UNCHANGED AsyncTransportClockTypeVars
           /\ UNCHANGED AsyncIngressTopologyTypeVars
           /\ UNCHANGED asyncIngressLanes
      BY <1>1, Isa
         DEF InjectByzantineCertifiedRequest, LeaveCausalQueues,
             AsyncRuntimeScalarTypeVars, AsyncIoVars, AsyncDeferredVars,
             AsyncIoTopologyTypeVars, AsyncIoContentTypeVars,
             AsyncIoCapacityTypeVars, AsyncDeferredTopologyTypeVars,
             AsyncTransportClockTypeVars,
             AsyncIngressTopologyTypeVars, AsyncSchedulerVars, vars
    <2>4. AsyncHistoricalRecoveryTypeInvariant'
      BY <1>1, HistoricalRecoveryFramePreservesType, Isa
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             InjectByzantineCertifiedRequest,
             AsyncHistoricalRecoveryFrameVars, vars
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4,
                 PublishTypedSingletonPreservesSchedulerType
  <1> QED BY <1>1

THEOREM AsyncFaultStepPreservesSchedulerType ==
  /\ StrongInductiveInvariant
  /\ AsyncTypeInvariant
  /\ AsyncFaultStep
  => AsyncSchedulerTypeInvariant'
PROOF
  <1>1. ASSUME StrongInductiveInvariant,
              AsyncTypeInvariant,
              AsyncFaultStep
         PROVE AsyncSchedulerTypeInvariant'
    <2>1. CASE \E packet \in asyncTransport:
                  PreGstLosePacket(packet)
      BY <1>1, <2>1, PreGstPacketLossPreservesSchedulerType
         DEF AsyncTypeInvariant
    <2>2. CASE \E node \in ValidatorIds: PreGstCrash(node)
      BY <1>1, <2>2, PreGstCrashPreservesSchedulerType
         DEF AsyncTypeInvariant
    <2>3. CASE \E source \in AsyncIngressSources,
                  recipient \in ValidatorIds,
                  nonce \in 0..(AsyncIngressCapacity - 1):
                  InjectByzantineNoise(source, recipient, nonce)
      BY <1>1, <2>3, InjectByzantineNoisePreservesSchedulerType
    <2>3c. CASE \E kind \in IngressTransportCompletionKinds,
                   recipient \in ValidatorIds,
                   nonce \in 0..(AsyncIngressCapacity - 1):
                   InjectUntrustedTransportCompletion(
                     kind, recipient, nonce)
      BY <1>1, <2>3c,
         InjectUntrustedTransportCompletionPreservesSchedulerType
    <2>4. CASE \E kind \in {"NormalJunk", "ProgressJunk"},
                  source \in ValidatorIds, recipient \in ValidatorIds,
                  nonce \in 0..(AsyncIngressCapacity - 1):
                  InjectAuthenticatedJunk(
                    kind, source, recipient, nonce)
      BY <1>1, <2>4,
         InjectAuthenticatedJunkPreservesSchedulerType
    <2>5. CASE \E source \in ValidatorIds,
                  recipient \in ValidatorIds, qc \in commitQCs,
                  nonce \in 0..(AsyncIngressCapacity - 1):
                  InjectByzantineCertifiedRequest(
                    source, recipient, qc, nonce)
      BY <1>1, <2>5,
         InjectByzantineCertifiedRequestPreservesSchedulerType
    <2>6. CASE \E signer \in ValidatorIds, roundView \in Views,
                  subject \in Subjects,
                  timeoutCertificate \in TimeoutCertificateOptionSet,
                  highestPrepare \in PrepareQcOptionSet:
                  AsyncByzantineProposal(
                    signer, roundView, subject,
                    timeoutCertificate, highestPrepare)
      BY <1>1, <2>6,
         AsyncByzantineProposalPreservesSchedulerType
    <2>7. CASE \E signer \in ValidatorIds, roundView \in Views,
                  phase \in Phases, subject \in Subjects:
                  AsyncByzantineVote(
                    signer, roundView, phase, subject)
      BY <1>1, <2>7, AsyncByzantineVotePreservesSchedulerType
    <2>8. CASE \E signer \in ValidatorIds, roundView \in Views,
                  highestPrepare \in PrepareQcOptionSet:
                  AsyncByzantineTimeout(
                    signer, roundView, highestPrepare)
      BY <1>1, <2>8,
         AsyncByzantineTimeoutPreservesSchedulerType
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>3c, <2>4,
                <2>5, <2>6, <2>7, <2>8
         DEF AsyncFaultStep, ByzantineProposalJustificationDomain
  <1> QED BY <1>1

THEOREM DirectCommitDiscoveryPreservesSchedulerType ==
  \A node \in ValidatorIds:
    /\ StrongInductiveInvariant
    /\ AsyncTypeInvariant
    /\ CommitCertificateDiscoveryStepWork(node)
    => AsyncSchedulerTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                StrongInductiveInvariant,
                AsyncTypeInvariant,
                CommitCertificateDiscoveryStepWork(node)
         PROVE AsyncSchedulerTypeInvariant'
    <2>1. node \in ValidatorIds
      BY <1>1
    <2>2. AsyncSchedulerTypeInvariant
      BY <1>1 DEF AsyncTypeInvariant
    <2>3. AsyncTransportContentTypeInvariant'
      BY <1>1, <2>1,
         DirectCommitDiscoveryPreservesTransportContentType
    <2>4. /\ UNCHANGED AsyncRuntimeScalarTypeVars
           /\ UNCHANGED asyncCausalQueues
           /\ UNCHANGED AsyncIoTopologyTypeVars
           /\ UNCHANGED AsyncIoContentTypeVars
           /\ UNCHANGED AsyncIoCapacityTypeVars
           /\ UNCHANGED AsyncDeferredTopologyTypeVars
           /\ UNCHANGED <<asyncDeferredCompletionQueues,
                           asyncDeferredProgressQueues,
                           asyncDeferredNormalQueues>>
           /\ UNCHANGED AsyncTransportClockTypeVars
           /\ UNCHANGED AsyncIngressTopologyTypeVars
           /\ UNCHANGED asyncIngressLanes
           /\ UNCHANGED AsyncHistoricalRecoveryFrameVars
      BY <1>1, Isa
         DEF CommitCertificateDiscoveryStepWork,
             PublishCommitCertificateRequests, LeaveCausalQueues,
             AsyncRuntimeScalarTypeVars, AsyncLocalAdmissionVars,
             AsyncIoTopologyTypeVars, AsyncIoContentTypeVars,
             AsyncIoCapacityTypeVars, AsyncIoVars,
             AsyncDeferredTopologyTypeVars, AsyncDeferredVars,
             AsyncTransportClockTypeVars,
             AsyncIngressTopologyTypeVars,
             AsyncHistoricalRecoveryFrameVars, vars
    <2>5. AsyncHistoricalRecoveryTypeInvariant'
      BY <2>2, <2>4, HistoricalRecoveryFramePreservesType
         DEF AsyncSchedulerTypeInvariant
    <2> QED BY <2>2, <2>3, <2>4, <2>5,
                AsyncTransportContentChangePreservesSchedulerType
  <1> QED BY <1>1

THEOREM OpenHistoricalRecoveryPreservesSchedulerType ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ OpenHistoricalRecovery(node)
    => AsyncSchedulerTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncTypeInvariant,
                OpenHistoricalRecovery(node)
         PROVE AsyncSchedulerTypeInvariant'
    <2>1. AsyncSchedulerTypeInvariant
      BY <1>1 DEF AsyncTypeInvariant
    <2>2. AsyncHistoricalRecoveryTypeInvariant'
      BY <1>1, ModelResponsiveValidators, SMTT(30)
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncHistoricalRecoveryTypeInvariant,
             OpenHistoricalRecovery, HistoricalRecoverySourceReady,
             HistoricalRecoveryTarget, NodeHasApplication,
             TypeInvariant, ModelConfiguration, QuorumConfiguration,
             AsyncSchedulerExceptHistoricalRecoveryTargets, vars
    <2>3. UNCHANGED
             <<context, AsyncSchedulerExceptHistoricalRecoveryTargets>>
      BY <1>1 DEF OpenHistoricalRecovery
    <2> QED BY <2>1, <2>2, <2>3,
                HistoricalRecoveryOnlyChangePreservesSchedulerType
  <1> QED BY <1>1

THEOREM AsyncNonRunnerStepPreservesSchedulerType ==
  /\ StrongInductiveInvariant
  /\ AsyncTypeInvariant
  /\ AsyncNonRunnerStep
  => AsyncSchedulerTypeInvariant'
PROOF
  <1>1. ASSUME StrongInductiveInvariant,
              AsyncTypeInvariant,
              AsyncNonRunnerStep
         PROVE AsyncSchedulerTypeInvariant'
    <2>1. CASE AsyncSetGST
      BY <1>1, <2>1, AsyncSetGstPreservesSchedulerType
         DEF AsyncTypeInvariant
    <2>2. CASE AsyncTick
      BY <1>1, <2>2, AsyncTickPreservesSchedulerType
         DEF AsyncTypeInvariant
    <2>3. CASE \E node \in ValidatorIds:
                  OpenHistoricalRecovery(node)
      BY <1>1, <2>3, OpenHistoricalRecoveryPreservesSchedulerType
    <2>4. CASE \E node \in AsyncCurrentResponsiveVoters:
                  DirectCommitCertificateDiscoveryStep(node)
      BY <1>1, <2>4, AsyncCurrentResponsiveVotersAreValidators,
         DirectCommitDiscoveryPreservesSchedulerType
         DEF DirectCommitCertificateDiscoveryStep
    <2>5. CASE \E node \in asyncHistoricalRecoveryTargets:
                  DirectHistoricalCommitCertificateDiscoveryStep(node)
      BY <1>1, <2>5, HistoricalRecoveryTargetsAreValidators,
         DirectCommitDiscoveryPreservesSchedulerType
         DEF DirectHistoricalCommitCertificateDiscoveryStep
    <2>6. CASE \E node \in AsyncArchiveIoServiceNodes:
                  ServiceIoWorker(node)
      BY <1>1, <2>6, AsyncArchiveIoServiceNodesAreValidators,
         ServiceIoWorkerPreservesSchedulerType
         DEF ServiceIoWorker
    <2>7. CASE \E node \in asyncHistoricalRecoveryTargets:
                  ServiceHistoricalRecoveryIoWorker(node)
      BY <1>1, <2>7, HistoricalRecoveryTargetsAreValidators,
         ServiceIoWorkerPreservesSchedulerType
         DEF ServiceHistoricalRecoveryIoWorker
    <2>8. CASE \E node \in AsyncCurrentResponsiveVoters:
                  EnqueueIoLocalControl(node)
      BY <1>1, <2>8, AsyncCurrentResponsiveVotersAreValidators,
         EnqueueIoControlPreservesSchedulerType
         DEF EnqueueIoLocalControl
    <2>9. CASE \E node \in asyncHistoricalRecoveryTargets:
                  EnqueueHistoricalRecoveryIoLocalControl(node)
      BY <1>1, <2>9, HistoricalRecoveryTargetsAreValidators,
         EnqueueIoControlPreservesSchedulerType
         DEF EnqueueHistoricalRecoveryIoLocalControl
    <2>10. CASE AsyncNetworkStep
      BY <1>1, <2>10, AsyncNetworkStepPreservesSchedulerType
    <2>11. CASE AsyncFaultStep
      BY <1>1, <2>11, AsyncFaultStepPreservesSchedulerType
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4, <2>5, <2>6,
                <2>7, <2>8, <2>9, <2>10, <2>11
         DEF AsyncNonRunnerStep
  <1> QED BY <1>1

THEOREM AsyncNetworkStepPreservesClaimIngressOwnership ==
  /\ AsyncTypeInvariant
  /\ AsyncCertifiedResponseClaimIngressOwnershipInvariant
  /\ AsyncNetworkStep
  => AsyncCertifiedResponseClaimIngressOwnershipInvariant'
BY AdmitIngressPacketPreservesClaimIngressOwnership
   DEF AsyncNetworkStep

THEOREM AsyncFaultStepClaimIngressStutter ==
  AsyncFaultStep
    => UNCHANGED
         <<asyncCertifiedResponseClaim, asyncIngressLanes>>
BY SMTT(90), Isa
   DEF AsyncFaultStep, PreGstLosePacket, PreGstCrash, Crash,
       InjectByzantineNoise, InjectUntrustedTransportCompletion,
       InjectAuthenticatedJunk, InjectByzantineCertifiedRequest,
       AsyncByzantineProposal, AsyncByzantineVote,
       AsyncByzantineTimeout,
       PublishControlItems, PublishEphemeralItems,
       PublishControlAndEphemeralItems,
       LeaveCausalQueues, AsyncDeferredVars,
       AsyncLocalAdmissionVars, AsyncSchedulerVars, vars

THEOREM AsyncNonRunnerStepPreservesClaimIngressOwnership ==
  /\ AsyncTypeInvariant
  /\ AsyncCertifiedResponseClaimIngressOwnershipInvariant
  /\ AsyncNonRunnerStep
  => AsyncCertifiedResponseClaimIngressOwnershipInvariant'
PROOF
  <1>1. ASSUME AsyncTypeInvariant,
              AsyncCertifiedResponseClaimIngressOwnershipInvariant,
              AsyncNonRunnerStep
         PROVE AsyncCertifiedResponseClaimIngressOwnershipInvariant'
    <2>1. CASE AsyncNetworkStep
      BY <1>1, <2>1,
         AsyncNetworkStepPreservesClaimIngressOwnership
    <2>2. CASE AsyncFaultStep
      BY <1>1, <2>2, AsyncFaultStepClaimIngressStutter,
         CertifiedResponseClaimIngressOwnershipStutter
    <2>3. CASE ~(AsyncNetworkStep \/ AsyncFaultStep)
      BY <1>1, <2>3,
         CertifiedResponseClaimIngressOwnershipStutter, Isa
         DEF AsyncNonRunnerStep, AsyncSetGST, AsyncTick,
             AsyncNonClockVars, OpenHistoricalRecovery,
             DirectCommitCertificateDiscoveryStep,
             DirectHistoricalCommitCertificateDiscoveryStep,
             CommitCertificateDiscoveryStepWork,
             PublishCommitCertificateRequests,
             ServiceIoWorker, ServiceHistoricalRecoveryIoWorker,
             ServiceIoWorkerWork, PublishEphemeralItems,
             EnqueueIoLocalControl, EnqueueHistoricalRecoveryIoLocalControl,
             EnqueueIoLocalControlWork, LeaveCausalQueues,
             AsyncDeferredVars, AsyncLocalAdmissionVars,
             AsyncSchedulerExceptHistoricalRecoveryTargets, vars
    <2> QED BY <1>1, <2>1, <2>2, <2>3
  <1> QED BY <1>1

=============================================================================
