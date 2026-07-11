---- MODULE SumeragiV2Reconfiguration ----
EXTENDS SumeragiV2CrashRecovery

(***************************************************************************
Height transition and epoch-context isolation.

A new context is created only after a common current-context decision has been
durably applied by the responsive quorum.  The new context binds the decided
parent subject, protocol/chain identifiers, finalized epoch roster and powers,
lane commitment, DA layout, and precomputed H(epoch_seed,height) leader start.
Old votes and certificates remain in history but fail QcValid/TCValid because
their context record differs from the current one.
***************************************************************************)

CommonAppliedSubject(subject) ==
  /\ subject \in Subjects
  /\ \A node \in Responsive \cap CurrentVoters:
       \E decision \in decisions:
         /\ decision.node = node
         /\ decision.qc.context = context
         /\ decision.qc.subject = subject
         /\ [node |-> node, qc |-> decision.qc] \in applied

AdvanceContext(subject) ==
  LET nextHeight == height + 1
      nextLineage == Append(context.lineage, subject)
      nextContext == ContextRecord(nextHeight, nextLineage)
  IN /\ height < MaxHeight
     /\ CommonAppliedSubject(subject)
     /\ height' = nextHeight
     /\ context' = nextContext
     /\ contextHistory' = contextHistory \cup {nextContext}
     /\ nodeView' = [node \in ValidatorIds |-> 0]
     /\ generation' =
          [node \in ValidatorIds |->
             IF generation[node] < MaxGeneration
             THEN generation[node] + 1 ELSE generation[node]]
     /\ lockRank' = [node \in ValidatorIds |-> NoRank]
     /\ lockSubject' = [node \in ValidatorIds |-> NoSubject]
     /\ highestRank' = [node \in ValidatorIds |-> NoRank]
     /\ highestSubject' = [node \in ValidatorIds |-> NoSubject]
     /\ availableBodies' = {}
     /\ validatedBodies' = {}
     /\ invalidBodies' = {}
     /\ seenProposals' = {}
     /\ receivedVotes' = {}
     /\ receivedQCs' = {}
     /\ receivedTimeoutVotes' = {}
     /\ receivedTCs' = {}
     /\ pendingProposal' = {}
     /\ pendingPrepare' = {}
     /\ pendingObservePrepare' = {}
     /\ pendingLockCommit' = {}
     /\ pendingTimeout' = {}
     /\ pendingInstallTC' = {}
     /\ pendingDecision' = {}
     /\ signProposals' = {}
     /\ signVotes' = {}
     /\ signTimeouts' = {}
     /\ proposalNetwork' = {}
     /\ voteNetwork' = {}
     /\ qcNetwork' = {}
     /\ timeoutNetwork' = {}
     /\ tcNetwork' = {}
     /\ UNCHANGED <<up, gst, durableBodies, proposalIntents,
                    prepareIntents, commitIntents, timeoutIntents,
                    prepareQCs, commitQCs, formedTCs, installedTCs,
                    decisions, applied>>

NextV2 == Next \/ \E subject \in Subjects: AdvanceContext(subject)

ReliableNextV2 ==
  ReliableNext \/ \E subject \in Subjects: AdvanceContext(subject)

RotationBoundConfiguration ==
  /\ MaxView >= N
  /\ \A contextValue \in ContextRecords:
       \E roundView \in 0..(Len(contextValue.roster) - 1):
         Leader(contextValue, roundView) \in
           Responsive \cap VotingRoster(contextValue.epoch)

OneRotationSuccessfulRoundBound(contextValue) ==
  Len(contextValue.roster) + 1

(***************************************************************************
Weak fairness is attached to each concrete, parameterized reducer action.
Fairness of the whole disjunction is insufficient: an unrelated continuously
enabled action could otherwise starve WAL acknowledgement, delivery, or apply
forever.  All quantifier domains below are finite, state-independent record
universes.  The two leader actions use `HonestProposalSubject`, which is either
the leader's certified lock or a deterministic valid empty-heartbeat subject.
***************************************************************************)
ReliableActionFairness ==
  /\ WF_vars(SetGST)
  /\ \A node \in ValidatorIds:
       /\ WF_vars(ReliableAssembleLocalBody(node))
       /\ WF_vars(ReliableBeginLocalProposal(node))
       /\ WF_vars(ReliableBeginTimeout(node))
  /\ \A request \in ProposalWalSet: WF_vars(PersistProposal(request))
  /\ \A request \in ProposalSignSet:
       WF_vars(CompleteProposalSignature(request))
  /\ \A envelope \in ProposalEnvelopeSet:
       WF_vars(DeliverProposal(envelope))
  /\ \A node \in ValidatorIds, proposal \in ProposalRecordSet:
       /\ WF_vars(FetchBody(node, proposal))
       /\ WF_vars(ValidateBody(node, proposal))
       /\ WF_vars(BeginPrepare(node, proposal))
  /\ \A node \in ValidatorIds, subject \in Subjects:
       WF_vars(StoreBody(node, subject))
  /\ \A request \in PrepareWalSet: WF_vars(PersistPrepare(request))
  /\ \A request \in VoteSignSet:
       WF_vars(CompleteVoteSignature(request))
  /\ \A envelope \in VoteEnvelopeSet: WF_vars(DeliverVote(envelope))
  /\ \A node \in ValidatorIds, roundView \in Views,
       subject \in Subjects:
       /\ WF_vars(FormPrepareQC(node, roundView, subject))
       /\ WF_vars(FormCommitQC(node, roundView, subject))
  /\ \A envelope \in QcEnvelopeSet: WF_vars(DeliverQC(envelope))
  /\ \A node \in ValidatorIds, qc \in QcRecordSet:
       /\ WF_vars(BeginObservePrepare(node, qc))
       /\ WF_vars(BeginLockCommit(node, qc))
       /\ WF_vars(BeginDecision(node, qc))
       /\ WF_vars(FetchCertifiedBody(node, qc))
       /\ WF_vars(ApplyDecision(node, qc))
  /\ \A request \in ObservePrepareWalSet:
       WF_vars(PersistObservePrepare(request))
  /\ \A request \in LockCommitWalSet:
       WF_vars(PersistLockCommit(request))
  /\ \A request \in DecisionWalSet: WF_vars(PersistDecision(request))
  /\ \A request \in TimeoutWalSet: WF_vars(PersistTimeout(request))
  /\ \A request \in TimeoutSignSet:
       WF_vars(CompleteTimeoutSignature(request))
  /\ \A envelope \in TimeoutEnvelopeSet:
       WF_vars(DeliverTimeout(envelope))
  /\ \A node \in ValidatorIds, roundView \in Views:
       WF_vars(FormTC(node, roundView))
  /\ \A envelope \in TcEnvelopeSet: WF_vars(DeliverTC(envelope))
  /\ \A node \in ValidatorIds, tc \in TcRecordSet:
       WF_vars(BeginInstallTC(node, tc))
  /\ \A request \in InstallTcWalSet: WF_vars(PersistInstallTC(request))
  /\ \A subject \in Subjects: WF_vars(AdvanceContext(subject))

Spec == Init /\ [][NextV2]_vars

LivenessSpec ==
  /\ Init
  /\ RotationBoundConfiguration
  /\ [][ReliableNextV2]_vars
  /\ ReliableActionFairness

ContextIdentityBindsParent ==
  \A blockHeight \in Heights:
    \A leftLineage, rightLineage \in LineagesAt(blockHeight):
      ContextRecord(blockHeight, leftLineage)
        = ContextRecord(blockHeight, rightLineage)
          => /\ leftLineage = rightLineage
             /\ ContextRecord(blockHeight, leftLineage).parentContextKey
                  = ContextRecord(blockHeight, rightLineage).parentContextKey
             /\ ContextRecord(blockHeight, leftLineage).parent
                  = ContextRecord(blockHeight, rightLineage).parent

ContextIdentityBindsFrozenEpoch ==
  \A contextValue \in ContextRecords:
    /\ contextValue.epoch = ExpectedEpoch(contextValue.height)
    /\ contextValue.roster = RosterSequence(contextValue.epoch)
    /\ contextValue.powers = EpochPowers[contextValue.epoch + 1]
    /\ contextValue.contextKey
         = ContextKey(contextValue.height, contextValue.lineage)
    /\ contextValue.parentContextKey
         = ParentContextKey(contextValue.height, contextValue.lineage)
    /\ contextValue.parentFinality
         = ParentFinalityIdentity(contextValue.height, contextValue.lineage)

OldContextCertificateRejected ==
  \A qc \in prepareQCs \cup commitQCs:
    qc.context # context => ~QcValid(qc)

ContextParentWasApplied ==
  \A contextValue \in contextHistory:
    contextValue.height > 0
      => \E decision \in decisions:
           /\ decision.qc.context.height + 1 = contextValue.height
           /\ decision.qc.subject = contextValue.parent
           /\ [node |-> decision.node, qc |-> decision.qc] \in applied

EpochBoundarySafety ==
  /\ ContextIdentityBindsFrozenEpoch
  /\ OldContextCertificateRejected
  /\ ContextParentWasApplied

THEOREM ContextRecordCarriesFrozenEpoch ==
  \A blockHeight \in Heights:
    \A lineage \in LineagesAt(blockHeight):
      ContextRecord(blockHeight, lineage).epoch = ExpectedEpoch(blockHeight)
BY DEF ContextRecord

THEOREM ContextRecordCarriesParent ==
  \A blockHeight \in Heights:
    \A lineage \in LineagesAt(blockHeight):
      /\ (blockHeight = 0
            => ContextRecord(blockHeight, lineage).parent = NoSubject)
      /\ (blockHeight > 0
            => ContextRecord(blockHeight, lineage).parent = lineage[blockHeight])
BY DEF ContextRecord

THEOREM ContextRecordCarriesParentContext ==
  \A blockHeight \in Heights:
    \A lineage \in LineagesAt(blockHeight):
      ContextRecord(blockHeight, lineage).parentContextKey
        = ParentContextKey(blockHeight, lineage)
BY DEF ContextRecord

THEOREM EquivalentParentCommitQcsConverge ==
  \A parentContextKey,
     parentHeight,
     parentSubject,
     leftView,
     rightView,
     leftSigners,
     rightSigners:
    SemanticParentFinality(
      CarriedParentCommit(parentContextKey, parentHeight, parentSubject,
                          leftView, leftSigners))
      = SemanticParentFinality(
          CarriedParentCommit(parentContextKey, parentHeight, parentSubject,
                              rightView, rightSigners))
BY DEF SemanticParentFinality, CarriedParentCommit

THEOREM ForeignParentLineageHasDifferentIdentity ==
  \A leftContextKey,
     rightContextKey,
     parentHeight,
     parentSubject,
     leftView,
     rightView,
     leftSigners,
     rightSigners:
    leftContextKey # rightContextKey
      => SemanticParentFinality(
           CarriedParentCommit(leftContextKey, parentHeight, parentSubject,
                               leftView, leftSigners))
           # SemanticParentFinality(
               CarriedParentCommit(rightContextKey, parentHeight,
                                   parentSubject, rightView, rightSigners))
BY DEF SemanticParentFinality, CarriedParentCommit

=============================================================================
