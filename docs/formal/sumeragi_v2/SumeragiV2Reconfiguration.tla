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
     /\ lastInstalledTc' =
          [node \in ValidatorIds |-> NoTimeoutCertificate]
     /\ lockPrepareQc' = [node \in ValidatorIds |-> NoPrepareQC]
     /\ highestPrepareQc' = [node \in ValidatorIds |-> NoPrepareQC]
     /\ lockRank' = [node \in ValidatorIds |-> NoRank]
     /\ lockSubject' = [node \in ValidatorIds |-> NoSubject]
     /\ highestRank' = [node \in ValidatorIds |-> NoRank]
     /\ highestSubject' = [node \in ValidatorIds |-> NoSubject]
     /\ availableBodies' = {}
     /\ retainedLockedBodies' = {}
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

Spec == Init /\ [][NextV2]_vars

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
    qc.context # context => ~QcWireValid(qc)

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

=============================================================================
