---- MODULE SumeragiV2TypedRolloverHandoff ----
EXTENDS Naturals, TLC

(***************************************************************************
Executable safety model for the move-only exact-output handoff and the
root-anchored merge-sidecar lifecycle V3 journal.

There are three, and only three, authorities for a changed-roster responder
generation:

* ordinary rehydration, which requires every predecessor responder stream,
  request gate, transfer, and flush to be terminal;
* the move-only durable exact-output handoff, which may supersede active
  requester-owned responder state because no predecessor writer or queued
  output can survive the sealed handoff; and
* restart recovery, which may supersede active state only after the V3 root
  marker and its exact selected snapshot have been validated.

An identical roster never advances the responder generation.  It either
preserves all retryable transport state or, when its bounded table is full,
returns Capacity without changing memory, the durable root, or either slot.

The service/transport owner pair is one-shot.  Its atomic seal belongs to the
baseline predecessor generation: a new pair cannot be created or sealed after
a successor authority has been restored or published.  A crash may clear the
process-local pair and reopen it only while recovery still selects that same
baseline predecessor.  This is the model boundary for the production
`sealed.compare_exchange(false, true)` ownership transition.

V3 publishes a checked successor into the inactive state slot, synchronizes
that slot, commits an exact root marker selecting it, and only then publishes
the successor projection to memory.  A crash before the root commit restores
the predecessor selected by the old root.  A crash after the root commit
restores the complete successor selected by the new root.  The root marker is
the model's local trust anchor; rollback or replacement of that marker
together with its matching slot is outside this model, just as it is outside
the journal's local rollback guarantee.

Forced fencing clears predecessor responder tables but never manufactures an
authenticated Close prefix.  Only AuthenticateServerClosePrefix can advance
the authentication history; every recorded retired prefix remains bounded by
that history.

Requester epochs remain durable-before-use.  Their V3 state-slot/root update
is abstracted as one atomic root commit because this model expands only the
rollover crash corridor.  The counter is the last-issued high-water used by
Rust, and the persisted snapshot carries the exact replacement incarnation.
A restart restores that incarnation; it does not skip the committed epoch.

The temporal predicate at the end remains a specification target.  This model
family does not claim an unbounded liveness or Rust-to-TLA refinement proof.
***************************************************************************)

NoIdentity == "NoIdentity"

Rosters == {"RosterA", "RosterB"}
CompactionCauses ==
  {"NoCompaction", "FullServerTable", "RosterGeometryReplacement"}
LifecycleEntryStates == {"Active", "Terminal", "Empty"}
ReceiptStages == {"Absent", "Minted", "Retained", "Lost", "Consumed"}
ChangedRosterAuthorities ==
  {"AuthenticatedTerminal", "DurableExactOutput", "RestartRestore"}
TransitionAuthorities ==
  {"None", "SameRosterPreserved"} \cup ChangedRosterAuthorities
PendingRolloverAuthorities == {"None"} \cup ChangedRosterAuthorities
LifecycleCommitPhases ==
  {"Bootstrap",
   "BootstrapStateReplaced",
   "BootstrapStatePublished",
   "BootstrapRootReplaced",
   "Current",
   "StateSlotReplaced",
   "StateSlotPublished",
   "RootReplaced",
   "RootCommitted",
   "Restarting",
   "Restored",
   "Published"}
RequesterEpochPhases ==
  {"Idle", "Persisted", "Restarting", "InUse", "Completed"}
StateSlots == {0, 1}
LifecycleRootShapes == {"Bootstrap", "Committed"}
StartupModes ==
  {"BootstrapStart",
   "LiveProcess",
   "UnvalidatedRestart",
   "ServiceGenerationLimitStart"}

LifecycleValidationFaults ==
  {"None",
   "LifecycleRootShapeMismatch",
   "LifecycleSelectedStateMissing",
   "LifecycleGenerationHashMismatch",
   "LifecycleSemanticValidationFailure"}

FailureReasons ==
  {"None",
   "LateExactOutputEnqueue",
   "ForeignOwnerMismatch",
   "PredecessorContextMismatch",
   "PredecessorArtifactMismatch",
   "ImmediateSuccessorMismatch",
   "LifecycleStateSlotV3PersistenceFailure",
   "LifecycleRootV3PersistenceFailure",
   "LifecycleRootGenerationOverflow",
   "CrashAfterBootstrapStateReplacement",
   "CrashAfterBootstrapStatePublication",
   "CrashAfterBootstrapRootReplacement",
   "CrashBeforeLifecycleStateSlotV3Publication",
   "CrashAfterLifecycleStateReplacement",
   "CrashAfterLifecycleStateSlotV3Publication",
   "CrashAfterLifecycleRootReplacement",
   "CrashAfterLifecycleRootV3Commit",
   "CrashAfterRequesterEpochPersistence",
   "LifecycleRootShapeMismatch",
   "LifecycleSelectedStateMissing",
   "LifecycleGenerationHashMismatch",
   "LifecycleSemanticValidationFailure"}

OwnerNonces == {"OwnerNonce", "ForeignNonce"}
HandoffCandidateKinds ==
  {"None",
   "ForeignOwner",
   "PredecessorContextMismatch",
   "PredecessorArtifactMismatch",
   "ImmediateSuccessorMismatch"}
Contexts == {"ContextA", "ContextB"}
Artifacts == {"ArtifactA", "ArtifactB"}
Parents == {"ParentA", "ParentB"}
Successors == {"SuccessorA", "SuccessorB"}

ForeignOwnerNonce == "ForeignNonce"
ExpectedContext == "ContextA"
ExpectedArtifact == "ArtifactA"
ExpectedParent == "ParentA"
ExpectedSuccessor == "SuccessorA"
ForeignSuccessor == "SuccessorB"

InitialWorkerOutstanding == 2
InitialServiceGeneration == 1
ServiceGenerationLimit == 2
InitialRootGeneration == 1
RootGenerationLimit == 6
InitialNextStreamEpoch == 0
StreamEpochLimit == 3
HighestSemanticSequence == 1
NoLifecycleSnapshot ==
  [version |-> 0,
   rootGeneration |-> 0,
   roster |-> NoIdentity,
   serviceGeneration |-> 0,
   nextStreamEpoch |-> 0,
   requesterStreamEpoch |-> 0,
   serverStreams |-> NoIdentity,
   requestGates |-> NoIdentity,
   serverClosePrefix |-> 0]

LifecycleSnapshotV3(
    rootGeneration,
    roster,
    serviceGeneration,
    nextStreamEpoch,
    requesterStreamEpoch,
    serverStreams,
    requestGates,
    serverClosePrefix) ==
  [version |-> 3,
   rootGeneration |-> rootGeneration,
   roster |-> roster,
   serviceGeneration |-> serviceGeneration,
   nextStreamEpoch |-> nextStreamEpoch,
   requesterStreamEpoch |-> requesterStreamEpoch,
   serverStreams |-> serverStreams,
   requestGates |-> requestGates,
   serverClosePrefix |-> serverClosePrefix]

LifecycleSnapshotV3Set ==
  [version: {3},
   rootGeneration: InitialRootGeneration..RootGenerationLimit,
   roster: Rosters,
   serviceGeneration: InitialServiceGeneration..ServiceGenerationLimit,
   nextStreamEpoch: InitialNextStreamEpoch..StreamEpochLimit,
   requesterStreamEpoch: InitialNextStreamEpoch..StreamEpochLimit,
   serverStreams: LifecycleEntryStates,
   requestGates: LifecycleEntryStates,
   serverClosePrefix: 0..HighestSemanticSequence]

LifecycleSnapshotDigest(snapshot) == snapshot

LifecycleStateSlot(rootGeneration) ==
  IF rootGeneration % 2 = 0 THEN 0 ELSE 1

BootstrapLifecycleRootV3 ==
  [version |-> 3,
   shape |-> "Bootstrap",
   rootGeneration |-> 0,
   snapshotDigest |-> NoLifecycleSnapshot]

LifecycleRootV3(snapshot) ==
  [version |-> 3,
   shape |-> "Committed",
   rootGeneration |-> snapshot.rootGeneration,
   snapshotDigest |-> LifecycleSnapshotDigest(snapshot)]

LifecycleRootV3Set ==
  [version: {3},
   shape: LifecycleRootShapes,
   rootGeneration: 0..RootGenerationLimit,
   snapshotDigest: LifecycleSnapshotV3Set \cup {NoLifecycleSnapshot}]

LifecycleStateSlotsV3Set ==
  [StateSlots -> LifecycleSnapshotV3Set \cup {NoLifecycleSnapshot}]

InitialLifecycleSnapshotV3(roster) ==
  LifecycleSnapshotV3(
    InitialRootGeneration,
    roster,
    InitialServiceGeneration,
    InitialNextStreamEpoch,
    InitialNextStreamEpoch,
    "Empty",
    "Empty",
    0)

LiveLifecycleSnapshotV3(roster, serviceGeneration) ==
  LifecycleSnapshotV3(
    InitialRootGeneration,
    roster,
    serviceGeneration,
    InitialNextStreamEpoch,
    InitialNextStreamEpoch,
    "Active",
    "Active",
    0)

InitialLifecycleStateSlotsV3(snapshot) ==
  [slot \in StateSlots |->
    IF slot = LifecycleStateSlot(snapshot.rootGeneration)
      THEN snapshot
      ELSE NoLifecycleSnapshot]

VARIABLE state

typedRolloverVars == <<state>>

LifecycleMemory(s) ==
  <<s.currentRoster,
    s.serviceGeneration,
    s.nextStreamEpoch,
    s.serverStreamState,
    s.requestGateState,
    s.serverClosePrefix,
    s.transferState,
    s.flushState>>

DurableLifecycle(s) ==
  <<s.durableLifecycleRootV3,
    s.durableLifecycleStateSlotsV3,
    s.syncedLifecycleRootV3,
    s.syncedLifecycleStateSlotsV3>>

CandidateLifecycle(s) ==
  <<s.candidatePresent,
    s.candidateStateSlot,
    s.candidateLifecycleSnapshotV3,
    s.candidateSemanticallyValidated,
    s.crashArtifactsPresent,
    s.cleanupPerformed,
    s.restartStateDirectoryResynced,
    s.restartRootDirectoryResynced>>

SelectedLifecycleStateSlot(s) ==
  LifecycleStateSlot(s.durableLifecycleRootV3.rootGeneration)

SelectedLifecycleSnapshotV3(s) ==
  s.durableLifecycleStateSlotsV3[SelectedLifecycleStateSlot(s)]

SyncedSelectedLifecycleStateSlot(s) ==
  LifecycleStateSlot(s.syncedLifecycleRootV3.rootGeneration)

SyncedSelectedLifecycleSnapshotV3(s) ==
  s.syncedLifecycleStateSlotsV3[
    SyncedSelectedLifecycleStateSlot(s)]

InactiveLifecycleStateSlot(s) ==
  1 - SelectedLifecycleStateSlot(s)

InactiveLifecycleArtifactPresent(s) ==
  /\ s.durableLifecycleRootV3.shape = "Committed"
  /\ s.durableLifecycleStateSlotsV3[
       InactiveLifecycleStateSlot(s)] # NoLifecycleSnapshot

LifecycleStateDirectoryIsSynced(s) ==
  s.durableLifecycleStateSlotsV3 =
    s.syncedLifecycleStateSlotsV3

LifecycleRootDirectoryIsSynced(s) ==
  s.durableLifecycleRootV3 = s.syncedLifecycleRootV3

LifecycleRootShapeIsValid(root) ==
  /\ (root.shape = "Bootstrap" =>
        /\ root.rootGeneration = 0
        /\ root.snapshotDigest = NoLifecycleSnapshot)
  /\ (root.shape = "Committed" =>
        /\ root.rootGeneration \in
             InitialRootGeneration..RootGenerationLimit
        /\ root.snapshotDigest # NoLifecycleSnapshot)

RootSelectedLifecyclePairIsPresent(s) ==
  /\ s.durableLifecycleRootV3.shape = "Committed"
  /\ SelectedLifecycleSnapshotV3(s) # NoLifecycleSnapshot

RootSelectedLifecyclePairMatches(s) ==
  /\ RootSelectedLifecyclePairIsPresent(s)
  /\ SelectedLifecycleSnapshotV3(s).rootGeneration =
       s.durableLifecycleRootV3.rootGeneration
  /\ s.durableLifecycleRootV3.snapshotDigest =
       LifecycleSnapshotDigest(SelectedLifecycleSnapshotV3(s))

SyncedRootSelectedLifecyclePairMatches(s) ==
  /\ s.syncedLifecycleRootV3.shape = "Committed"
  /\ SyncedSelectedLifecycleSnapshotV3(s) # NoLifecycleSnapshot
  /\ SyncedSelectedLifecycleSnapshotV3(s).rootGeneration =
       s.syncedLifecycleRootV3.rootGeneration
  /\ s.syncedLifecycleRootV3.snapshotDigest =
       LifecycleSnapshotDigest(
         SyncedSelectedLifecycleSnapshotV3(s))

LifecycleSnapshotSemanticallyValid(snapshot) ==
  /\ snapshot \in LifecycleSnapshotV3Set
  /\ snapshot.version = 3
  /\ snapshot.rootGeneration # 0
  /\ snapshot.serverClosePrefix <= HighestSemanticSequence
  /\ snapshot.requesterStreamEpoch <= snapshot.nextStreamEpoch

DurableSnapshot(s) ==
  SelectedLifecycleSnapshotV3(s)

SyncedLifecycleSnapshot(s) ==
  SyncedSelectedLifecycleSnapshotV3(s)

LifecycleJournalReady(s) ==
  /\ s.durableJournalValidated
  /\ RootSelectedLifecyclePairMatches(s)
  /\ LifecycleSnapshotSemanticallyValid(DurableSnapshot(s))
  /\ LifecycleStateDirectoryIsSynced(s)
  /\ LifecycleRootDirectoryIsSynced(s)
  /\ ~s.crashArtifactsPresent

ExactServiceTransportOwnerPair ==
  /\ state.serviceOwnerNonce # NoIdentity
  /\ state.serviceOwnerNonce = state.transportOwnerNonce

PredecessorTransportOwnershipOpen ==
  /\ state.currentRoster = state.baselineRoster
  /\ state.transitionAuthority = "None"

ExactPredecessorReceipt ==
  /\ ExactServiceTransportOwnerPair
  /\ state.receiptOwnerNonce = state.serviceOwnerNonce
  /\ state.receiptContext = ExpectedContext
  /\ state.receiptArtifact = ExpectedArtifact

ExactSuccessorConstruction ==
  /\ state.constructionParent = ExpectedParent
  /\ state.constructionSuccessor = ExpectedSuccessor

ExactRetainedMergeSidecars ==
  /\ state.receiptStage = "Retained"
  /\ ExactPredecessorReceipt
  /\ ExactSuccessorConstruction
  /\ state.retainedSuccessor = ExpectedSuccessor

FinalExactOutputSeal ==
  /\ state.finalityValidated
  /\ state.workerIngressClosed
  /\ state.workerOutstanding = 0
  /\ state.ownerSealed

CompactionCauseMatchesGeometry ==
  /\ (state.targetRoster # state.currentRoster =>
        state.compactionCause = "RosterGeometryReplacement")
  /\ (state.compactionCause = "RosterGeometryReplacement" =>
        state.targetRoster # state.currentRoster)
  /\ (state.compactionCause = "FullServerTable" =>
        state.targetRoster = state.currentRoster)

ChangedRosterReplacementNeeded ==
  /\ state.targetRoster # state.currentRoster
  /\ state.compactionCause = "RosterGeometryReplacement"

SameRosterTableFull ==
  /\ state.targetRoster = state.currentRoster
  /\ state.compactionCause = "FullServerTable"

AllOldLifecycleTerminal ==
  /\ state.serverStreamState = "Terminal"
  /\ state.requestGateState = "Terminal"
  /\ state.transferState = "Terminal"
  /\ state.flushState = "Terminal"

PersistentLifecycleMemoryMatchesSnapshot(snapshot) ==
  /\ state.currentRoster = snapshot.roster
  /\ state.serviceGeneration = snapshot.serviceGeneration
  /\ state.nextStreamEpoch = snapshot.nextStreamEpoch
  /\ state.serverStreamState = snapshot.serverStreams
  /\ state.requestGateState = snapshot.requestGates
  /\ state.serverClosePrefix = snapshot.serverClosePrefix

LifecycleMemoryMatchesDurableSnapshotV3 ==
  PersistentLifecycleMemoryMatchesSnapshot(DurableSnapshot(state))

LifecycleTablesMatchDurableSnapshotV3 ==
  /\ state.currentRoster = DurableSnapshot(state).roster
  /\ state.serviceGeneration = DurableSnapshot(state).serviceGeneration
  /\ state.serverStreamState = DurableSnapshot(state).serverStreams
  /\ state.requestGateState = DurableSnapshot(state).requestGates
  /\ state.serverClosePrefix = DurableSnapshot(state).serverClosePrefix

DurableCandidateStateSlotAheadOfRoot ==
  /\ state.candidatePresent
  /\ state.lifecycleCommitPhase \in
       {"StateSlotReplaced", "StateSlotPublished"}
  /\ state.candidateStateSlot =
       LifecycleStateSlot(
         state.durableLifecycleRootV3.rootGeneration + 1)
  /\ state.candidateStateSlot #
       SelectedLifecycleStateSlot(state)
  /\ state.durableLifecycleStateSlotsV3[
       state.candidateStateSlot] =
       state.candidateLifecycleSnapshotV3
  /\ LifecycleSnapshotSemanticallyValid(
       state.candidateLifecycleSnapshotV3)
  /\ state.candidateLifecycleSnapshotV3.rootGeneration =
       state.durableLifecycleRootV3.rootGeneration + 1
  /\ state.candidateLifecycleSnapshotV3.roster = state.targetRoster
  /\ state.candidateLifecycleSnapshotV3.serviceGeneration =
       state.serviceGeneration + 1
  /\ state.candidateLifecycleSnapshotV3.nextStreamEpoch =
       state.nextStreamEpoch
  /\ state.candidateLifecycleSnapshotV3.requesterStreamEpoch =
       DurableSnapshot(state).requesterStreamEpoch
  /\ state.candidateLifecycleSnapshotV3.serverStreams = "Empty"
  /\ state.candidateLifecycleSnapshotV3.requestGates = "Empty"
  /\ state.candidateLifecycleSnapshotV3.serverClosePrefix = 0

ValidatedCandidateSuccessorStateSlotAheadOfRoot ==
  /\ DurableCandidateStateSlotAheadOfRoot
  /\ state.candidateSemanticallyValidated

RootCommittedSuccessorAheadOfMemory ==
  /\ state.lifecycleCommitPhase = "RootCommitted"
  /\ state.candidatePresent
  /\ LifecycleRootDirectoryIsSynced(state)
  /\ LifecycleStateDirectoryIsSynced(state)
  /\ DurableSnapshot(state) = state.candidateLifecycleSnapshotV3
  /\ SelectedLifecycleStateSlot(state) = state.candidateStateSlot
  /\ RootSelectedLifecyclePairMatches(state)
  /\ LifecycleSnapshotSemanticallyValid(DurableSnapshot(state))
  /\ DurableSnapshot(state).serviceGeneration =
       state.serviceGeneration + 1
  /\ DurableSnapshot(state).serverStreams = "Empty"
  /\ DurableSnapshot(state).requestGates = "Empty"
  /\ DurableSnapshot(state).serverClosePrefix = 0

RootSelectedSuccessorAheadOfMemory ==
  /\ state.durableLifecycleRootV3.shape = "Committed"
  /\ RootSelectedLifecyclePairMatches(state)
  /\ LifecycleSnapshotSemanticallyValid(DurableSnapshot(state))
  /\ DurableSnapshot(state).serviceGeneration =
       state.serviceGeneration + 1
  /\ DurableSnapshot(state).serverStreams = "Empty"
  /\ DurableSnapshot(state).requestGates = "Empty"
  /\ DurableSnapshot(state).serverClosePrefix = 0

RequesterEpochPersistenceAheadOfMemory ==
  /\ state.requesterEpochPhase = "Persisted"
  /\ state.pendingStreamEpoch = state.nextStreamEpoch + 1
  /\ DurableSnapshot(state).nextStreamEpoch =
       state.pendingStreamEpoch
  /\ DurableSnapshot(state).requesterStreamEpoch =
       state.pendingStreamEpoch

CrashClearedProcessLocalRolloverState(s) ==
  [s EXCEPT
     !.finalityValidated = FALSE,
     !.workerIngressClosed = FALSE,
     !.workerOutstanding = InitialWorkerOutstanding,
     !.ownerSealed = FALSE,
     !.serviceOwnerNonce = NoIdentity,
     !.transportOwnerNonce = NoIdentity,
     !.constructionParent = NoIdentity,
     !.constructionSuccessor = NoIdentity,
     !.presentedHandoffCandidate = "None",
     !.receiptStage =
       IF s.receiptStage = "Absent" THEN "Absent" ELSE "Lost",
     !.receiptOwnerNonce = NoIdentity,
     !.receiptContext = NoIdentity,
     !.receiptArtifact = NoIdentity,
     !.retainedSuccessor = NoIdentity,
     !.receiptConsumeCount = 0,
     !.candidatePresent = FALSE,
     !.candidateSemanticallyValidated = FALSE,
     !.pendingRolloverAuthority = "None",
     !.successorActive = FALSE,
     !.transitionAuthority = "None",
     !.restartFenceAuthorized = FALSE]

ChangedRosterAuthorityAvailable(authority) ==
  CASE authority = "AuthenticatedTerminal" ->
         /\ ExactRetainedMergeSidecars
         /\ AllOldLifecycleTerminal
    [] authority = "DurableExactOutput" ->
         /\ ExactRetainedMergeSidecars
         /\ state.durableJournalValidated
    [] authority = "RestartRestore" ->
         /\ state.durableJournalValidated
         /\ state.validatedRestartObserved
         /\ state.restartFenceAuthorized
         /\ state.receiptStage \in {"Absent", "Lost"}

Init ==
  \E current \in Rosters,
     target \in Rosters,
     cause \in CompactionCauses,
     startup \in StartupModes,
     fault \in LifecycleValidationFaults:
    /\ (target # current =>
          cause = "RosterGeometryReplacement")
    /\ (cause = "RosterGeometryReplacement" =>
          target # current)
    /\ (cause = "FullServerTable" =>
          target = current)
    /\ (cause = "NoCompaction" =>
          target = current)
    /\ (startup = "BootstrapStart" =>
          /\ target = current
          /\ cause = "NoCompaction"
          /\ fault = "None")
    /\ (startup = "ServiceGenerationLimitStart" =>
          /\ target # current
          /\ cause = "RosterGeometryReplacement"
          /\ fault = "None")
    /\ (startup # "UnvalidatedRestart" => fault = "None")
    /\ LET initialServiceGeneration ==
             IF startup = "ServiceGenerationLimitStart"
               THEN ServiceGenerationLimit
               ELSE InitialServiceGeneration
           initialSnapshot ==
             IF startup = "BootstrapStart"
               THEN InitialLifecycleSnapshotV3(target)
               ELSE LiveLifecycleSnapshotV3(
                      current, initialServiceGeneration)
       IN state =
         [startupMode |-> startup,
          startupValidationFault |-> fault,
          startupValidationRejected |-> FALSE,
          validationFailureObserved |-> FALSE,
          targetRoster |-> target,
          compactionCause |-> cause,
          currentRoster |-> initialSnapshot.roster,
          baselineRoster |-> initialSnapshot.roster,
          baselineServiceGeneration |->
            initialSnapshot.serviceGeneration,
          finalityValidated |-> FALSE,
          workerIngressClosed |-> FALSE,
          workerOutstanding |-> InitialWorkerOutstanding,
          ownerSealed |-> FALSE,
          serviceOwnerNonce |-> NoIdentity,
          transportOwnerNonce |-> NoIdentity,
          constructionParent |-> NoIdentity,
          constructionSuccessor |-> NoIdentity,
          presentedHandoffCandidate |-> "None",
          receiptStage |-> "Absent",
          receiptOwnerNonce |-> NoIdentity,
          receiptContext |-> NoIdentity,
          receiptArtifact |-> NoIdentity,
          retainedSuccessor |-> NoIdentity,
          receiptConsumeCount |-> 0,
          serviceGeneration |-> initialSnapshot.serviceGeneration,
          nextStreamEpoch |-> initialSnapshot.nextStreamEpoch,
          requesterEpochPhase |-> "Idle",
          pendingStreamEpoch |-> 0,
          activeStreamEpoch |-> 0,
          lastCompletedStreamEpoch |-> 0,
          requesterEpochReplacementRestored |-> FALSE,
          serverStreamState |-> initialSnapshot.serverStreams,
          requestGateState |-> initialSnapshot.requestGates,
          serverClosePrefix |-> initialSnapshot.serverClosePrefix,
          authenticatedCloseHistory |-> 0,
          recordedRetiredClosePrefix |-> 0,
          transferState |->
            IF startup = "BootstrapStart" THEN "Empty" ELSE "Active",
          flushState |->
            IF startup = "BootstrapStart" THEN "Empty" ELSE "Active",
          durableLifecycleRootV3 |->
            IF startup = "BootstrapStart"
              THEN BootstrapLifecycleRootV3
              ELSE LifecycleRootV3(initialSnapshot),
          durableLifecycleStateSlotsV3 |->
            IF startup = "BootstrapStart"
              THEN [slot \in StateSlots |-> NoLifecycleSnapshot]
              ELSE InitialLifecycleStateSlotsV3(initialSnapshot),
          syncedLifecycleRootV3 |->
            IF startup = "BootstrapStart"
              THEN BootstrapLifecycleRootV3
              ELSE LifecycleRootV3(initialSnapshot),
          syncedLifecycleStateSlotsV3 |->
            IF startup = "BootstrapStart"
              THEN [slot \in StateSlots |-> NoLifecycleSnapshot]
              ELSE InitialLifecycleStateSlotsV3(initialSnapshot),
          candidateLifecycleSnapshotV3 |-> initialSnapshot,
          candidatePresent |-> FALSE,
          candidateStateSlot |-> 0,
          candidateSemanticallyValidated |-> FALSE,
          lifecycleCommitPhase |->
            IF startup = "BootstrapStart"
              THEN "Bootstrap"
              ELSE IF startup = "UnvalidatedRestart"
                THEN "Restarting"
                ELSE "Current",
          pendingRolloverAuthority |-> "None",
          durableJournalValidated |->
            startup # "UnvalidatedRestart",
          validatedRestartObserved |-> FALSE,
          restartFenceAuthorized |-> FALSE,
          crashArtifactsPresent |->
            startup = "UnvalidatedRestart",
          cleanupPerformed |-> FALSE,
          restartStateDirectoryResynced |-> FALSE,
          restartRootDirectoryResynced |-> FALSE,
          retryableChunk |->
            IF startup = "BootstrapStart" THEN 0 ELSE 1,
          successorActive |-> FALSE,
          transitionAuthority |-> "None",
          capacityRejected |-> FALSE,
          restartRequired |->
            startup = "UnvalidatedRestart",
          failureReason |-> "None",
          lateEnqueueRejected |-> FALSE,
          foreignReceiptRejected |-> FALSE,
          predecessorMismatchRejected |-> FALSE,
          wrongSuccessorRejected |-> FALSE,
          lateOldCallbackObserved |-> FALSE,
          bootstrapCrashObserved |-> FALSE,
          stateSlotCrashObserved |-> FALSE,
          rootCrashObserved |-> FALSE,
          epochCrashObserved |-> FALSE,
          secondCrashObserved |-> FALSE]

CreateServiceTransportOwnerPair ==
  /\ PredecessorTransportOwnershipOpen
  /\ state.serviceOwnerNonce = NoIdentity
  /\ state.transportOwnerNonce = NoIdentity
  /\ state.receiptStage \in {"Absent", "Lost"}
  /\ ~state.restartRequired
  /\ ~state.successorActive
  /\ state' =
       [state EXCEPT
          !.serviceOwnerNonce = "OwnerNonce",
          !.transportOwnerNonce = "OwnerNonce",
          !.receiptStage = "Absent",
          !.restartFenceAuthorized = FALSE]

ValidateFinality ==
  /\ ~state.finalityValidated
  /\ ~state.successorActive
  /\ ~state.restartRequired
  /\ state' = [state EXCEPT !.finalityValidated = TRUE]

CloseWorkerIngress ==
  /\ state.finalityValidated
  /\ ~state.workerIngressClosed
  /\ ~state.successorActive
  /\ ~state.restartRequired
  /\ state' = [state EXCEPT !.workerIngressClosed = TRUE]

ClearOneWorkerExactOutput ==
  /\ state.workerIngressClosed
  /\ state.workerOutstanding > 0
  /\ ~state.successorActive
  /\ ~state.restartRequired
  /\ state' =
       [state EXCEPT !.workerOutstanding = @ - 1]

BuildImmediateSuccessor ==
  /\ state.finalityValidated
  /\ state.constructionParent = NoIdentity
  /\ state.constructionSuccessor = NoIdentity
  /\ ~state.successorActive
  /\ ~state.restartRequired
  /\ state' =
       [state EXCEPT
          !.constructionParent = ExpectedParent,
          !.constructionSuccessor = ExpectedSuccessor]

SealAppliedHeightOutputHandoff ==
  /\ PredecessorTransportOwnershipOpen
  /\ state.finalityValidated
  /\ state.workerIngressClosed
  /\ state.workerOutstanding = 0
  /\ ~state.ownerSealed
  /\ state.receiptStage = "Absent"
  /\ ExactServiceTransportOwnerPair
  /\ ~state.successorActive
  /\ ~state.restartRequired
  /\ state' =
       [state EXCEPT
          !.ownerSealed = TRUE,
          !.receiptStage = "Minted",
          !.receiptOwnerNonce = state.serviceOwnerNonce,
          !.receiptContext = ExpectedContext,
          !.receiptArtifact = ExpectedArtifact]

RejectLateExactOutputEnqueue ==
  /\ state.ownerSealed
  /\ state.receiptStage = "Minted"
  /\ ~state.successorActive
  /\ ~state.restartRequired
  /\ state' =
       [state EXCEPT
          !.lateEnqueueRejected = TRUE,
          !.restartRequired = TRUE,
          !.failureReason = "LateExactOutputEnqueue"]

PresentForeignOwnerReceiptCandidate ==
  /\ state.receiptStage = "Minted"
  /\ ExactSuccessorConstruction
  /\ state.presentedHandoffCandidate = "None"
  /\ ~state.restartRequired
  /\ state' =
       [state EXCEPT
          !.presentedHandoffCandidate = "ForeignOwner"]

PresentPredecessorContextMismatchCandidate ==
  /\ state.receiptStage = "Minted"
  /\ ExactSuccessorConstruction
  /\ state.presentedHandoffCandidate = "None"
  /\ ~state.restartRequired
  /\ state' =
       [state EXCEPT
          !.presentedHandoffCandidate =
            "PredecessorContextMismatch"]

PresentPredecessorArtifactMismatchCandidate ==
  /\ state.receiptStage = "Minted"
  /\ ExactSuccessorConstruction
  /\ state.presentedHandoffCandidate = "None"
  /\ ~state.restartRequired
  /\ state' =
       [state EXCEPT
          !.presentedHandoffCandidate =
            "PredecessorArtifactMismatch"]

PresentWrongImmediateSuccessorCandidate ==
  /\ state.receiptStage = "Minted"
  /\ ExactSuccessorConstruction
  /\ state.presentedHandoffCandidate = "None"
  /\ ~state.restartRequired
  /\ state' =
       [state EXCEPT
          !.presentedHandoffCandidate =
            "ImmediateSuccessorMismatch"]

RejectForeignOwnerReceipt ==
  /\ state.receiptStage = "Minted"
  /\ ExactSuccessorConstruction
  /\ state.presentedHandoffCandidate = "ForeignOwner"
  /\ ~state.foreignReceiptRejected
  /\ ~state.successorActive
  /\ ~state.restartRequired
  /\ state' =
       [state EXCEPT
          !.foreignReceiptRejected = TRUE,
          !.restartRequired = TRUE,
          !.failureReason = "ForeignOwnerMismatch"]

RejectPredecessorContextMismatch ==
  /\ state.receiptStage = "Minted"
  /\ ExactSuccessorConstruction
  /\ state.presentedHandoffCandidate =
       "PredecessorContextMismatch"
  /\ ~state.predecessorMismatchRejected
  /\ ~state.successorActive
  /\ ~state.restartRequired
  /\ state' =
       [state EXCEPT
          !.predecessorMismatchRejected = TRUE,
          !.restartRequired = TRUE,
          !.failureReason = "PredecessorContextMismatch"]

RejectPredecessorArtifactMismatch ==
  /\ state.receiptStage = "Minted"
  /\ ExactSuccessorConstruction
  /\ state.presentedHandoffCandidate =
       "PredecessorArtifactMismatch"
  /\ ~state.predecessorMismatchRejected
  /\ ~state.successorActive
  /\ ~state.restartRequired
  /\ state' =
       [state EXCEPT
          !.predecessorMismatchRejected = TRUE,
          !.restartRequired = TRUE,
          !.failureReason = "PredecessorArtifactMismatch"]

RejectWrongImmediateSuccessor ==
  /\ state.receiptStage = "Minted"
  /\ ExactSuccessorConstruction
  /\ state.presentedHandoffCandidate =
       "ImmediateSuccessorMismatch"
  /\ ~state.wrongSuccessorRejected
  /\ ~state.successorActive
  /\ ~state.restartRequired
  /\ state' =
       [state EXCEPT
          !.wrongSuccessorRejected = TRUE,
          !.restartRequired = TRUE,
          !.failureReason = "ImmediateSuccessorMismatch"]

RetainExactHandoffReceipt ==
  /\ state.receiptStage = "Minted"
  /\ ExactPredecessorReceipt
  /\ ExactSuccessorConstruction
  /\ state.presentedHandoffCandidate = "None"
  /\ state.retainedSuccessor = NoIdentity
  /\ ~state.successorActive
  /\ ~state.restartRequired
  /\ state' =
       [state EXCEPT
          !.receiptStage = "Retained",
          !.retainedSuccessor = ExpectedSuccessor]

(***************************************************************************
Bootstrap and restart validation.  The generation-zero sentinel is the only
root shape without a selected state.  Generation one is written to its parity
slot, semantically validated, and only then selected by the committed root.
Known crash artifacts may be retired only after the root-selected pair and its
transport semantics have both been validated.
***************************************************************************)
PublishInitialLifecycleStateSlotV3 ==
  /\ state.lifecycleCommitPhase = "Bootstrap"
  /\ state.durableLifecycleRootV3 = BootstrapLifecycleRootV3
  /\ state.syncedLifecycleRootV3 = BootstrapLifecycleRootV3
  /\ LifecycleStateDirectoryIsSynced(state)
  /\ ~state.candidatePresent
  /\ ~state.restartRequired
  /\ LET snapshot ==
           InitialLifecycleSnapshotV3(state.targetRoster)
     IN state' =
       [state EXCEPT
          !.durableLifecycleStateSlotsV3[
            LifecycleStateSlot(snapshot.rootGeneration)] =
              snapshot,
          !.candidateLifecycleSnapshotV3 = snapshot,
          !.candidatePresent = TRUE,
          !.candidateStateSlot =
            LifecycleStateSlot(snapshot.rootGeneration),
          !.candidateSemanticallyValidated = FALSE,
          !.lifecycleCommitPhase = "BootstrapStateReplaced",
          !.crashArtifactsPresent = TRUE,
          !.cleanupPerformed = FALSE]

SyncInitialLifecycleStateDirectoryV3 ==
  /\ state.lifecycleCommitPhase = "BootstrapStateReplaced"
  /\ state.durableLifecycleRootV3 = BootstrapLifecycleRootV3
  /\ state.candidatePresent
  /\ ~state.candidateSemanticallyValidated
  /\ ~LifecycleStateDirectoryIsSynced(state)
  /\ state' =
       [state EXCEPT
          !.syncedLifecycleStateSlotsV3 =
            state.durableLifecycleStateSlotsV3,
          !.lifecycleCommitPhase = "BootstrapStatePublished"]

CrashAfterBootstrapStateReplacement ==
  /\ state.lifecycleCommitPhase = "BootstrapStateReplaced"
  /\ state.durableLifecycleRootV3 = BootstrapLifecycleRootV3
  /\ ~LifecycleStateDirectoryIsSynced(state)
  /\ ~state.restartRequired
  /\ \E replacementSurvives \in BOOLEAN:
       state' =
         [CrashClearedProcessLocalRolloverState(state) EXCEPT
            !.durableLifecycleStateSlotsV3 =
              IF replacementSurvives
                THEN state.durableLifecycleStateSlotsV3
                ELSE state.syncedLifecycleStateSlotsV3,
            !.durableLifecycleRootV3 =
              state.syncedLifecycleRootV3,
            !.lifecycleCommitPhase = "Restarting",
            !.durableJournalValidated = FALSE,
            !.validatedRestartObserved = FALSE,
            !.restartStateDirectoryResynced = FALSE,
            !.restartRootDirectoryResynced = FALSE,
            !.crashArtifactsPresent = TRUE,
            !.cleanupPerformed = FALSE,
            !.restartRequired = TRUE,
            !.failureReason =
              "CrashAfterBootstrapStateReplacement",
            !.bootstrapCrashObserved = TRUE]

CrashAfterBootstrapStatePublication ==
  /\ state.lifecycleCommitPhase = "BootstrapStatePublished"
  /\ LifecycleStateDirectoryIsSynced(state)
  /\ ~state.restartRequired
  /\ state' =
       [CrashClearedProcessLocalRolloverState(state) EXCEPT
          !.lifecycleCommitPhase = "Restarting",
          !.durableJournalValidated = FALSE,
          !.validatedRestartObserved = FALSE,
          !.restartStateDirectoryResynced = FALSE,
          !.restartRootDirectoryResynced = FALSE,
          !.crashArtifactsPresent = TRUE,
          !.cleanupPerformed = FALSE,
          !.restartRequired = TRUE,
          !.failureReason =
            "CrashAfterBootstrapStatePublication",
          !.bootstrapCrashObserved = TRUE]

ValidateBootstrapLifecycleCandidateV3 ==
  /\ state.lifecycleCommitPhase \in
       {"BootstrapStatePublished", "Restarting"}
  /\ state.durableLifecycleRootV3 = BootstrapLifecycleRootV3
  /\ state.startupValidationFault = "None"
  /\ state.durableLifecycleStateSlotsV3[
       LifecycleStateSlot(InitialRootGeneration)] =
       InitialLifecycleSnapshotV3(state.targetRoster)
  /\ LifecycleSnapshotSemanticallyValid(
       InitialLifecycleSnapshotV3(state.targetRoster))
  /\ state' =
       [state EXCEPT
          !.candidateLifecycleSnapshotV3 =
            InitialLifecycleSnapshotV3(state.targetRoster),
          !.candidatePresent = TRUE,
          !.candidateStateSlot =
            LifecycleStateSlot(InitialRootGeneration),
          !.candidateSemanticallyValidated = TRUE,
          !.lifecycleCommitPhase = "BootstrapStatePublished",
          !.durableJournalValidated = TRUE,
          !.validatedRestartObserved =
            state.validatedRestartObserved
              \/ state.restartRequired]

ValidateBootstrapLifecycleWithoutCandidateV3 ==
  /\ state.lifecycleCommitPhase = "Restarting"
  /\ state.durableLifecycleRootV3 = BootstrapLifecycleRootV3
  /\ state.startupValidationFault = "None"
  /\ state.durableLifecycleStateSlotsV3[
       LifecycleStateSlot(InitialRootGeneration)] =
       NoLifecycleSnapshot
  /\ ~state.durableJournalValidated
  /\ state' =
       [state EXCEPT
          !.durableJournalValidated = TRUE,
          !.validatedRestartObserved = TRUE]

ReplaceInitialLifecycleRootV3 ==
  /\ state.lifecycleCommitPhase = "BootstrapStatePublished"
  /\ state.durableLifecycleRootV3 = BootstrapLifecycleRootV3
  /\ state.candidatePresent
  /\ state.candidateSemanticallyValidated
  /\ LifecycleStateDirectoryIsSynced(state)
  /\ state.candidateLifecycleSnapshotV3.rootGeneration =
       InitialRootGeneration
  /\ state' =
       [state EXCEPT
          !.durableLifecycleRootV3 =
            LifecycleRootV3(state.candidateLifecycleSnapshotV3),
          !.lifecycleCommitPhase = "BootstrapRootReplaced"]

CrashAfterBootstrapRootReplacement ==
  /\ state.lifecycleCommitPhase = "BootstrapRootReplaced"
  /\ ~LifecycleRootDirectoryIsSynced(state)
  /\ ~state.restartRequired
  /\ \E replacementSurvives \in BOOLEAN:
       state' =
         [CrashClearedProcessLocalRolloverState(state) EXCEPT
            !.durableLifecycleRootV3 =
              IF replacementSurvives
                THEN state.durableLifecycleRootV3
                ELSE state.syncedLifecycleRootV3,
            !.lifecycleCommitPhase = "Restarting",
            !.durableJournalValidated = FALSE,
            !.validatedRestartObserved = FALSE,
            !.restartStateDirectoryResynced = FALSE,
            !.restartRootDirectoryResynced = FALSE,
            !.crashArtifactsPresent = TRUE,
            !.cleanupPerformed = FALSE,
            !.restartRequired = TRUE,
            !.failureReason =
              "CrashAfterBootstrapRootReplacement",
            !.bootstrapCrashObserved = TRUE]

CommitInitialLifecycleRootV3 ==
  /\ state.lifecycleCommitPhase = "BootstrapRootReplaced"
  /\ state.candidatePresent
  /\ state.candidateSemanticallyValidated
  /\ RootSelectedLifecyclePairMatches(state)
  /\ LifecycleStateDirectoryIsSynced(state)
  /\ ~LifecycleRootDirectoryIsSynced(state)
  /\ state' =
       [state EXCEPT
          !.syncedLifecycleRootV3 =
            state.durableLifecycleRootV3,
          !.candidatePresent = FALSE,
          !.candidateSemanticallyValidated = FALSE,
          !.lifecycleCommitPhase = "Current",
          !.durableJournalValidated = TRUE,
          !.restartStateDirectoryResynced = FALSE,
          !.restartRootDirectoryResynced = FALSE,
          !.crashArtifactsPresent = FALSE,
          !.cleanupPerformed = TRUE,
          !.restartRequired = FALSE,
          !.failureReason = "None"]

ValidateRootSelectedLifecycleV3 ==
  /\ state.restartRequired
  /\ ~state.durableJournalValidated
  /\ state.startupValidationFault = "None"
  /\ LifecycleRootShapeIsValid(state.durableLifecycleRootV3)
  /\ RootSelectedLifecyclePairMatches(state)
  /\ LifecycleSnapshotSemanticallyValid(DurableSnapshot(state))
  /\ state' =
       [state EXCEPT
          !.durableJournalValidated = TRUE,
          !.validatedRestartObserved = TRUE,
          !.restartFenceAuthorized = FALSE]

RejectInvalidLifecycleStartupV3(reason) ==
  /\ state.restartRequired
  /\ ~state.durableJournalValidated
  /\ state.startupValidationFault = reason
  /\ reason \in LifecycleValidationFaults \ {"None"}
  /\ state.crashArtifactsPresent
  /\ ~state.cleanupPerformed
  /\ ~state.startupValidationRejected
  /\ state' =
       [state EXCEPT
          !.startupValidationRejected = TRUE,
          !.validationFailureObserved = TRUE,
          !.failureReason = reason]

RejectLifecycleRootShapeMismatchV3 ==
  RejectInvalidLifecycleStartupV3(
    "LifecycleRootShapeMismatch")

RejectLifecycleSelectedStateMissingV3 ==
  RejectInvalidLifecycleStartupV3(
    "LifecycleSelectedStateMissing")

RejectLifecycleGenerationHashMismatchV3 ==
  RejectInvalidLifecycleStartupV3(
    "LifecycleGenerationHashMismatch")

RejectLifecycleSemanticValidationFailureV3 ==
  RejectInvalidLifecycleStartupV3(
    "LifecycleSemanticValidationFailure")

ResyncValidatedLifecycleStateDirectoryV3 ==
  /\ state.restartRequired
  /\ state.durableJournalValidated
  /\ state.lifecycleCommitPhase \in
       {"Restarting", "BootstrapStatePublished"}
  /\ ~state.restartStateDirectoryResynced
  /\ state' =
       [state EXCEPT
          !.syncedLifecycleStateSlotsV3 =
            state.durableLifecycleStateSlotsV3,
          !.restartStateDirectoryResynced = TRUE]

ResyncValidatedLifecycleRootDirectoryV3 ==
  /\ state.restartRequired
  /\ state.durableJournalValidated
  /\ state.lifecycleCommitPhase \in
       {"Restarting", "BootstrapStatePublished"}
  /\ state.restartStateDirectoryResynced
  /\ ~state.restartRootDirectoryResynced
  /\ state' =
       [state EXCEPT
          !.syncedLifecycleRootV3 =
            state.durableLifecycleRootV3,
          !.restartRootDirectoryResynced = TRUE]

CrashDuringValidatedRestartBeforeRootResyncV3 ==
  /\ state.restartRequired
  /\ state.durableJournalValidated
  /\ state.restartStateDirectoryResynced
  /\ ~state.restartRootDirectoryResynced
  /\ state.crashArtifactsPresent
  /\ state' =
       [CrashClearedProcessLocalRolloverState(state) EXCEPT
          !.durableLifecycleRootV3 =
            state.syncedLifecycleRootV3,
          !.durableLifecycleStateSlotsV3 =
            state.syncedLifecycleStateSlotsV3,
          !.lifecycleCommitPhase = "Restarting",
          !.durableJournalValidated = FALSE,
          !.validatedRestartObserved = FALSE,
          !.restartStateDirectoryResynced = FALSE,
          !.restartRootDirectoryResynced = FALSE,
          !.cleanupPerformed = FALSE,
          !.failureReason =
            IF state.failureReason = "None"
              THEN "CrashAfterLifecycleRootReplacement"
              ELSE state.failureReason,
          !.secondCrashObserved = TRUE]

CleanupValidatedLifecycleArtifactsV3 ==
  /\ state.restartRequired
  /\ state.durableJournalValidated
  /\ RootSelectedLifecyclePairMatches(state)
  /\ LifecycleSnapshotSemanticallyValid(DurableSnapshot(state))
  /\ state.restartStateDirectoryResynced
  /\ state.restartRootDirectoryResynced
  /\ LifecycleStateDirectoryIsSynced(state)
  /\ LifecycleRootDirectoryIsSynced(state)
  /\ state.durableLifecycleRootV3.shape = "Committed"
  /\ state.crashArtifactsPresent
  /\ ~state.cleanupPerformed
  /\ state' =
       [state EXCEPT
          !.durableLifecycleStateSlotsV3[
            InactiveLifecycleStateSlot(state)] =
              NoLifecycleSnapshot,
          !.syncedLifecycleStateSlotsV3[
            InactiveLifecycleStateSlot(state)] =
              NoLifecycleSnapshot,
          !.candidatePresent = FALSE,
          !.candidateSemanticallyValidated = FALSE,
          !.crashArtifactsPresent = FALSE,
          !.cleanupPerformed = TRUE]

CompleteBootstrapRestartWithoutCandidateV3 ==
  /\ state.restartRequired
  /\ state.durableJournalValidated
  /\ state.durableLifecycleRootV3 = BootstrapLifecycleRootV3
  /\ state.restartStateDirectoryResynced
  /\ state.restartRootDirectoryResynced
  /\ state.durableLifecycleStateSlotsV3[
       LifecycleStateSlot(InitialRootGeneration)] =
       NoLifecycleSnapshot
  /\ state' =
       [state EXCEPT
          !.lifecycleCommitPhase = "Bootstrap",
          !.crashArtifactsPresent = FALSE,
          !.cleanupPerformed = TRUE,
          !.restartRequired = FALSE,
          !.failureReason = "None"]

(***************************************************************************
Requester epoch allocation.  The V3 root-selected counter advances before the
following publication action may expose the allocated epoch for use.
***************************************************************************)
PersistFreshRequesterEpoch ==
  /\ state.requesterEpochPhase = "Idle"
  /\ state.nextStreamEpoch < StreamEpochLimit
  /\ state.durableLifecycleRootV3.rootGeneration < RootGenerationLimit
  /\ state.lifecycleCommitPhase = "Current"
  /\ ~state.candidatePresent
  /\ LifecycleJournalReady(state)
  /\ LifecycleMemoryMatchesDurableSnapshotV3
  /\ ~state.restartRequired
  /\ LET issuedEpoch == state.nextStreamEpoch + 1
         snapshot ==
           LifecycleSnapshotV3(
             state.durableLifecycleRootV3.rootGeneration + 1,
             state.currentRoster,
             state.serviceGeneration,
             issuedEpoch,
             issuedEpoch,
             state.serverStreamState,
             state.requestGateState,
             state.serverClosePrefix)
     IN state' =
          [state EXCEPT
             !.durableLifecycleStateSlotsV3[
               SelectedLifecycleStateSlot(state)] =
                 NoLifecycleSnapshot,
             !.durableLifecycleStateSlotsV3[
               LifecycleStateSlot(snapshot.rootGeneration)] =
                 snapshot,
             !.syncedLifecycleStateSlotsV3[
               SelectedLifecycleStateSlot(state)] =
                 NoLifecycleSnapshot,
             !.syncedLifecycleStateSlotsV3[
               LifecycleStateSlot(snapshot.rootGeneration)] =
                 snapshot,
             !.durableLifecycleRootV3 =
               LifecycleRootV3(snapshot),
             !.syncedLifecycleRootV3 =
               LifecycleRootV3(snapshot),
             !.requesterEpochPhase = "Persisted",
             !.pendingStreamEpoch = issuedEpoch,
             !.crashArtifactsPresent = FALSE,
             !.cleanupPerformed = TRUE,
             !.capacityRejected = FALSE]

PublishFreshRequesterEpoch ==
  /\ RequesterEpochPersistenceAheadOfMemory
  /\ ~state.restartRequired
  /\ state.pendingStreamEpoch > state.lastCompletedStreamEpoch
  /\ state' =
       [state EXCEPT
          !.nextStreamEpoch = DurableSnapshot(state).nextStreamEpoch,
          !.requesterEpochPhase = "InUse",
          !.activeStreamEpoch = state.pendingStreamEpoch,
          !.pendingStreamEpoch = 0]

CompleteRequesterEpoch ==
  /\ state.requesterEpochPhase = "InUse"
  /\ state.activeStreamEpoch > state.lastCompletedStreamEpoch
  /\ state' =
       [state EXCEPT
          !.requesterEpochPhase = "Completed",
          !.lastCompletedStreamEpoch = state.activeStreamEpoch,
          !.activeStreamEpoch = 0]

ReopenRequesterEpochAllocator ==
  /\ state.requesterEpochPhase = "Completed"
  /\ state' =
       [state EXCEPT !.requesterEpochPhase = "Idle"]

CrashAfterRequesterEpochPersistence ==
  /\ RequesterEpochPersistenceAheadOfMemory
  /\ ~state.restartRequired
  /\ state' =
       [CrashClearedProcessLocalRolloverState(state) EXCEPT
          !.lifecycleCommitPhase = "Restarting",
          !.requesterEpochPhase = "Restarting",
          !.pendingStreamEpoch = 0,
          !.activeStreamEpoch = 0,
          !.requesterEpochReplacementRestored = FALSE,
          !.durableJournalValidated = FALSE,
          !.validatedRestartObserved = FALSE,
          !.restartStateDirectoryResynced = FALSE,
          !.restartRootDirectoryResynced = FALSE,
          !.crashArtifactsPresent = TRUE,
          !.cleanupPerformed = FALSE,
          !.restartRequired = TRUE,
          !.failureReason = "CrashAfterRequesterEpochPersistence",
          !.epochCrashObserved = TRUE]

RestoreRequesterEpochCounterAfterCrash ==
  /\ state.restartRequired
  /\ state.failureReason = "CrashAfterRequesterEpochPersistence"
  /\ state.epochCrashObserved
  /\ state.durableJournalValidated
  /\ state.cleanupPerformed
  /\ state.restartStateDirectoryResynced
  /\ state.restartRootDirectoryResynced
  /\ RootSelectedLifecyclePairMatches(state)
  /\ LifecycleSnapshotSemanticallyValid(DurableSnapshot(state))
  /\ DurableSnapshot(state).nextStreamEpoch =
       state.nextStreamEpoch + 1
  /\ DurableSnapshot(state).requesterStreamEpoch =
       DurableSnapshot(state).nextStreamEpoch
  /\ state' =
       [state EXCEPT
          !.nextStreamEpoch = DurableSnapshot(state).nextStreamEpoch,
          !.requesterEpochPhase = "InUse",
          !.pendingStreamEpoch = 0,
          !.activeStreamEpoch =
            DurableSnapshot(state).requesterStreamEpoch,
          !.requesterEpochReplacementRestored = TRUE,
          !.lifecycleCommitPhase = "Current",
          !.validatedRestartObserved = TRUE,
          !.restartFenceAuthorized =
            state.targetRoster # state.currentRoster,
          !.crashArtifactsPresent = FALSE,
          !.cleanupPerformed = TRUE,
          !.restartRequired = FALSE,
          !.failureReason = "None"]

RejectRequesterEpochOverflow ==
  /\ state.requesterEpochPhase = "Idle"
  /\ state.nextStreamEpoch = StreamEpochLimit
  /\ ~state.capacityRejected
  /\ state' =
       [state EXCEPT !.capacityRejected = TRUE]

(***************************************************************************
Ordinary terminalization.  Only an authenticated Close advances the semantic
prefix.  Transfer and flush ownership is process-local, but both must also be
terminal before the ordinary changed-roster API can advance a generation.
***************************************************************************)
RejectActiveOrdinaryRollover ==
  /\ ExactRetainedMergeSidecars
  /\ ChangedRosterReplacementNeeded
  /\ ~AllOldLifecycleTerminal
  /\ ~state.capacityRejected
  /\ state.lifecycleCommitPhase = "Current"
  /\ ~state.restartRequired
  /\ state' =
       [state EXCEPT !.capacityRejected = TRUE]

RejectSameRosterFullTable ==
  /\ ExactRetainedMergeSidecars
  /\ SameRosterTableFull
  /\ ~state.capacityRejected
  /\ state.lifecycleCommitPhase = "Current"
  /\ ~state.restartRequired
  /\ state' =
       [state EXCEPT !.capacityRejected = TRUE]

AuthenticateServerClosePrefix ==
  /\ ExactRetainedMergeSidecars
  /\ ChangedRosterReplacementNeeded
  /\ state.serverStreamState = "Active"
  /\ state.durableLifecycleRootV3.rootGeneration < RootGenerationLimit
  /\ state.lifecycleCommitPhase = "Current"
  /\ state.requesterEpochPhase # "Persisted"
  /\ LifecycleJournalReady(state)
  /\ LifecycleMemoryMatchesDurableSnapshotV3
  /\ ~state.restartRequired
  /\ LET snapshot ==
           LifecycleSnapshotV3(
             state.durableLifecycleRootV3.rootGeneration + 1,
             state.currentRoster,
             state.serviceGeneration,
             state.nextStreamEpoch,
             DurableSnapshot(state).requesterStreamEpoch,
             "Terminal",
             state.requestGateState,
             HighestSemanticSequence)
     IN state' =
          [state EXCEPT
             !.serverStreamState = "Terminal",
             !.serverClosePrefix = HighestSemanticSequence,
             !.authenticatedCloseHistory = HighestSemanticSequence,
             !.durableLifecycleStateSlotsV3[
               SelectedLifecycleStateSlot(state)] =
                 NoLifecycleSnapshot,
             !.durableLifecycleStateSlotsV3[
               LifecycleStateSlot(snapshot.rootGeneration)] =
                 snapshot,
             !.syncedLifecycleStateSlotsV3[
               SelectedLifecycleStateSlot(state)] =
                 NoLifecycleSnapshot,
             !.syncedLifecycleStateSlotsV3[
               LifecycleStateSlot(snapshot.rootGeneration)] =
                 snapshot,
             !.durableLifecycleRootV3 =
               LifecycleRootV3(snapshot),
             !.syncedLifecycleRootV3 =
               LifecycleRootV3(snapshot),
             !.cleanupPerformed = TRUE,
             !.capacityRejected = FALSE]

TerminalizeRequestGate ==
  /\ ExactRetainedMergeSidecars
  /\ ChangedRosterReplacementNeeded
  /\ state.requestGateState = "Active"
  /\ state.durableLifecycleRootV3.rootGeneration < RootGenerationLimit
  /\ state.lifecycleCommitPhase = "Current"
  /\ state.requesterEpochPhase # "Persisted"
  /\ LifecycleJournalReady(state)
  /\ LifecycleMemoryMatchesDurableSnapshotV3
  /\ ~state.restartRequired
  /\ LET snapshot ==
           LifecycleSnapshotV3(
             state.durableLifecycleRootV3.rootGeneration + 1,
             state.currentRoster,
             state.serviceGeneration,
             state.nextStreamEpoch,
             DurableSnapshot(state).requesterStreamEpoch,
             state.serverStreamState,
             "Terminal",
             state.serverClosePrefix)
     IN state' =
          [state EXCEPT
             !.requestGateState = "Terminal",
             !.durableLifecycleStateSlotsV3[
               SelectedLifecycleStateSlot(state)] =
                 NoLifecycleSnapshot,
             !.durableLifecycleStateSlotsV3[
               LifecycleStateSlot(snapshot.rootGeneration)] =
                 snapshot,
             !.syncedLifecycleStateSlotsV3[
               SelectedLifecycleStateSlot(state)] =
                 NoLifecycleSnapshot,
             !.syncedLifecycleStateSlotsV3[
               LifecycleStateSlot(snapshot.rootGeneration)] =
                 snapshot,
             !.durableLifecycleRootV3 =
               LifecycleRootV3(snapshot),
             !.syncedLifecycleRootV3 =
               LifecycleRootV3(snapshot),
             !.cleanupPerformed = TRUE,
             !.capacityRejected = FALSE]

TerminalizeTransfer ==
  /\ ExactRetainedMergeSidecars
  /\ ChangedRosterReplacementNeeded
  /\ state.transferState = "Active"
  /\ ~state.restartRequired
  /\ state' =
       [state EXCEPT
          !.transferState = "Terminal",
          !.capacityRejected = FALSE]

TerminalizeFlush ==
  /\ ExactRetainedMergeSidecars
  /\ ChangedRosterReplacementNeeded
  /\ state.flushState = "Active"
  /\ ~state.restartRequired
  /\ state' =
       [state EXCEPT
          !.flushState = "Terminal",
          !.capacityRejected = FALSE]

(***************************************************************************
Changed-roster V3 commit corridor.  The state-slot publication is not a
commit.  The root marker binds the exact snapshot and slot.  Memory changes
only after the root-selected successor is durable.
***************************************************************************)
PublishSuccessorLifecycleStateSlotV3WithAuthority(authority) ==
  /\ authority \in ChangedRosterAuthorities
  /\ ChangedRosterReplacementNeeded
  /\ ChangedRosterAuthorityAvailable(authority)
  /\ state.serviceGeneration < ServiceGenerationLimit
  /\ state.durableLifecycleRootV3.rootGeneration <
       RootGenerationLimit
  /\ state.lifecycleCommitPhase = "Current"
  /\ ~state.candidatePresent
  /\ state.requesterEpochPhase # "Persisted"
  /\ LifecycleJournalReady(state)
  /\ LifecycleMemoryMatchesDurableSnapshotV3
  /\ ~state.restartRequired
  /\ LET snapshot ==
           LifecycleSnapshotV3(
             state.durableLifecycleRootV3.rootGeneration + 1,
             state.targetRoster,
             state.serviceGeneration + 1,
             state.nextStreamEpoch,
             DurableSnapshot(state).requesterStreamEpoch,
             "Empty",
             "Empty",
             0)
     IN state' =
       [state EXCEPT
          !.durableLifecycleStateSlotsV3[
            LifecycleStateSlot(snapshot.rootGeneration)] =
              snapshot,
          !.candidateLifecycleSnapshotV3 = snapshot,
          !.candidatePresent = TRUE,
          !.candidateStateSlot =
            LifecycleStateSlot(snapshot.rootGeneration),
          !.candidateSemanticallyValidated = TRUE,
          !.lifecycleCommitPhase = "StateSlotReplaced",
          !.pendingRolloverAuthority = authority,
          !.crashArtifactsPresent = TRUE,
          !.cleanupPerformed = FALSE,
          !.capacityRejected = FALSE]

PublishSuccessorLifecycleStateSlotV3 ==
  \E authority \in ChangedRosterAuthorities:
    PublishSuccessorLifecycleStateSlotV3WithAuthority(authority)

PublishDurableExactOutputSuccessorLifecycleStateSlotV3 ==
  PublishSuccessorLifecycleStateSlotV3WithAuthority(
    "DurableExactOutput")

PublishRestartRestoreSuccessorLifecycleStateSlotV3 ==
  PublishSuccessorLifecycleStateSlotV3WithAuthority(
    "RestartRestore")

SyncSuccessorLifecycleStateDirectoryV3 ==
  /\ ValidatedCandidateSuccessorStateSlotAheadOfRoot
  /\ state.lifecycleCommitPhase = "StateSlotReplaced"
  /\ ~LifecycleStateDirectoryIsSynced(state)
  /\ ~state.restartRequired
  /\ state' =
       [state EXCEPT
          !.syncedLifecycleStateSlotsV3 =
            state.durableLifecycleStateSlotsV3,
          !.lifecycleCommitPhase = "StateSlotPublished"]

FailSuccessorLifecycleStateSlotV3Persistence ==
  /\ ChangedRosterReplacementNeeded
  /\ ExactRetainedMergeSidecars
  /\ state.serviceGeneration < ServiceGenerationLimit
  /\ state.durableLifecycleRootV3.rootGeneration < RootGenerationLimit
  /\ state.lifecycleCommitPhase = "Current"
  /\ ~state.candidatePresent
  /\ LifecycleJournalReady(state)
  /\ LifecycleMemoryMatchesDurableSnapshotV3
  /\ ~state.restartRequired
  /\ state' =
       [CrashClearedProcessLocalRolloverState(state) EXCEPT
          !.lifecycleCommitPhase = "Restarting",
          !.durableJournalValidated = FALSE,
          !.validatedRestartObserved = FALSE,
          !.restartStateDirectoryResynced = FALSE,
          !.restartRootDirectoryResynced = FALSE,
          !.crashArtifactsPresent = TRUE,
          !.cleanupPerformed = FALSE,
          !.restartRequired = TRUE,
          !.failureReason = "LifecycleStateSlotV3PersistenceFailure"]

CrashBeforeLifecycleStateSlotV3Publication ==
  /\ ChangedRosterReplacementNeeded
  /\ ExactRetainedMergeSidecars
  /\ state.lifecycleCommitPhase = "Current"
  /\ ~state.candidatePresent
  /\ LifecycleMemoryMatchesDurableSnapshotV3
  /\ ~state.restartRequired
  /\ state' =
       [CrashClearedProcessLocalRolloverState(state) EXCEPT
          !.lifecycleCommitPhase = "Restarting",
          !.durableJournalValidated = FALSE,
          !.validatedRestartObserved = FALSE,
          !.restartStateDirectoryResynced = FALSE,
          !.restartRootDirectoryResynced = FALSE,
          !.crashArtifactsPresent = TRUE,
          !.cleanupPerformed = FALSE,
          !.restartRequired = TRUE,
          !.failureReason = "CrashBeforeLifecycleStateSlotV3Publication"]

CrashAfterLifecycleStateReplacement ==
  /\ ValidatedCandidateSuccessorStateSlotAheadOfRoot
  /\ state.lifecycleCommitPhase = "StateSlotReplaced"
  /\ ~LifecycleStateDirectoryIsSynced(state)
  /\ ~state.restartRequired
  /\ \E replacementSurvives \in BOOLEAN:
       state' =
         [CrashClearedProcessLocalRolloverState(state) EXCEPT
            !.durableLifecycleStateSlotsV3 =
              IF replacementSurvives
                THEN state.durableLifecycleStateSlotsV3
                ELSE state.syncedLifecycleStateSlotsV3,
            !.durableLifecycleRootV3 =
              state.syncedLifecycleRootV3,
            !.lifecycleCommitPhase = "Restarting",
            !.durableJournalValidated = FALSE,
            !.validatedRestartObserved = FALSE,
            !.restartStateDirectoryResynced = FALSE,
            !.restartRootDirectoryResynced = FALSE,
            !.crashArtifactsPresent = TRUE,
            !.cleanupPerformed = FALSE,
            !.restartRequired = TRUE,
            !.failureReason =
              "CrashAfterLifecycleStateReplacement",
            !.stateSlotCrashObserved = TRUE]

CrashAfterLifecycleStateSlotV3Publication ==
  /\ ValidatedCandidateSuccessorStateSlotAheadOfRoot
  /\ state.lifecycleCommitPhase = "StateSlotPublished"
  /\ LifecycleStateDirectoryIsSynced(state)
  /\ ~state.restartRequired
  /\ state' =
       [CrashClearedProcessLocalRolloverState(state) EXCEPT
          !.lifecycleCommitPhase = "Restarting",
          !.durableJournalValidated = FALSE,
          !.validatedRestartObserved = FALSE,
          !.restartStateDirectoryResynced = FALSE,
          !.restartRootDirectoryResynced = FALSE,
          !.crashArtifactsPresent = TRUE,
          !.cleanupPerformed = FALSE,
          !.restartRequired = TRUE,
          !.failureReason = "CrashAfterLifecycleStateSlotV3Publication",
          !.stateSlotCrashObserved = TRUE]

FailSuccessorLifecycleRootV3Persistence ==
  /\ ValidatedCandidateSuccessorStateSlotAheadOfRoot
  /\ state.lifecycleCommitPhase = "StateSlotPublished"
  /\ LifecycleStateDirectoryIsSynced(state)
  /\ ~state.restartRequired
  /\ state' =
       [CrashClearedProcessLocalRolloverState(state) EXCEPT
          !.lifecycleCommitPhase = "Restarting",
          !.durableJournalValidated = FALSE,
          !.validatedRestartObserved = FALSE,
          !.restartStateDirectoryResynced = FALSE,
          !.restartRootDirectoryResynced = FALSE,
          !.crashArtifactsPresent = TRUE,
          !.cleanupPerformed = FALSE,
          !.restartRequired = TRUE,
          !.failureReason = "LifecycleRootV3PersistenceFailure",
          !.stateSlotCrashObserved = TRUE]

ReplaceSuccessorLifecycleRootV3 ==
  /\ ValidatedCandidateSuccessorStateSlotAheadOfRoot
  /\ state.lifecycleCommitPhase = "StateSlotPublished"
  /\ LifecycleStateDirectoryIsSynced(state)
  /\ LifecycleRootDirectoryIsSynced(state)
  /\ state.pendingRolloverAuthority \in
       ChangedRosterAuthorities
  /\ ~state.restartRequired
  /\ state' =
       [state EXCEPT
          !.durableLifecycleRootV3 =
            LifecycleRootV3(
              state.candidateLifecycleSnapshotV3),
          !.lifecycleCommitPhase = "RootReplaced"]

CrashAfterLifecycleRootReplacement ==
  /\ state.lifecycleCommitPhase = "RootReplaced"
  /\ state.candidatePresent
  /\ RootSelectedLifecyclePairMatches(state)
  /\ ~LifecycleRootDirectoryIsSynced(state)
  /\ ~state.restartRequired
  /\ \E replacementSurvives \in BOOLEAN:
       state' =
         [CrashClearedProcessLocalRolloverState(state) EXCEPT
            !.durableLifecycleRootV3 =
              IF replacementSurvives
                THEN state.durableLifecycleRootV3
                ELSE state.syncedLifecycleRootV3,
            !.lifecycleCommitPhase = "Restarting",
            !.durableJournalValidated = FALSE,
            !.validatedRestartObserved = FALSE,
            !.restartStateDirectoryResynced = FALSE,
            !.restartRootDirectoryResynced = FALSE,
            !.crashArtifactsPresent = TRUE,
            !.cleanupPerformed = FALSE,
            !.restartRequired = TRUE,
            !.failureReason =
              "CrashAfterLifecycleRootReplacement",
            !.rootCrashObserved = TRUE]

RecoverPredecessorLifecycleV3 ==
  /\ state.restartRequired
  /\ state.durableJournalValidated
  /\ state.cleanupPerformed
  /\ state.restartStateDirectoryResynced
  /\ state.restartRootDirectoryResynced
  /\ RootSelectedLifecyclePairMatches(state)
  /\ LifecycleSnapshotSemanticallyValid(DurableSnapshot(state))
  /\ LifecycleMemoryMatchesDurableSnapshotV3
  /\ state' =
       [state EXCEPT
          !.candidatePresent = FALSE,
          !.candidateSemanticallyValidated = FALSE,
          !.lifecycleCommitPhase = "Current",
          !.pendingRolloverAuthority = "None",
          !.validatedRestartObserved = TRUE,
          !.restartFenceAuthorized =
            state.targetRoster # state.currentRoster,
          !.crashArtifactsPresent = FALSE,
          !.cleanupPerformed = TRUE,
          !.restartRequired = FALSE,
          !.failureReason = "None"]

CommitSuccessorLifecycleRootV3 ==
  /\ state.lifecycleCommitPhase = "RootReplaced"
  /\ state.candidatePresent
  /\ RootSelectedLifecyclePairMatches(state)
  /\ LifecycleSnapshotSemanticallyValid(DurableSnapshot(state))
  /\ LifecycleStateDirectoryIsSynced(state)
  /\ ~LifecycleRootDirectoryIsSynced(state)
  /\ state.pendingRolloverAuthority \in ChangedRosterAuthorities
  /\ ~state.restartRequired
  /\ state' =
       [state EXCEPT
          !.syncedLifecycleRootV3 =
            state.durableLifecycleRootV3,
          !.lifecycleCommitPhase = "RootCommitted"]

CleanupCommittedLifecyclePredecessorV3 ==
  /\ state.lifecycleCommitPhase \in {"RootCommitted", "Published"}
  /\ RootSelectedLifecyclePairMatches(state)
  /\ LifecycleStateDirectoryIsSynced(state)
  /\ LifecycleRootDirectoryIsSynced(state)
  /\ InactiveLifecycleArtifactPresent(state)
  /\ ~state.restartRequired
  /\ state' =
       [state EXCEPT
          !.durableLifecycleStateSlotsV3[
            InactiveLifecycleStateSlot(state)] =
              NoLifecycleSnapshot,
          !.syncedLifecycleStateSlotsV3[
            InactiveLifecycleStateSlot(state)] =
              NoLifecycleSnapshot,
          !.crashArtifactsPresent = FALSE,
          !.cleanupPerformed = TRUE]

CrashAfterLifecycleRootV3Commit ==
  /\ RootCommittedSuccessorAheadOfMemory
  /\ ~state.restartRequired
  /\ state' =
       [CrashClearedProcessLocalRolloverState(state) EXCEPT
          !.lifecycleCommitPhase = "Restarting",
          !.durableJournalValidated = FALSE,
          !.validatedRestartObserved = FALSE,
          !.restartStateDirectoryResynced = FALSE,
          !.restartRootDirectoryResynced = FALSE,
          !.crashArtifactsPresent = TRUE,
          !.cleanupPerformed = FALSE,
          !.restartRequired = TRUE,
          !.failureReason = "CrashAfterLifecycleRootV3Commit",
          !.rootCrashObserved = TRUE]

RestoreSuccessorLifecycleV3AfterCrash ==
  /\ RootSelectedSuccessorAheadOfMemory
  /\ state.restartRequired
  /\ state.durableJournalValidated
  /\ state.cleanupPerformed
  /\ state.restartStateDirectoryResynced
  /\ state.restartRootDirectoryResynced
  /\ RootSelectedLifecyclePairMatches(state)
  /\ LifecycleSnapshotSemanticallyValid(DurableSnapshot(state))
  /\ state' =
       [state EXCEPT
          !.currentRoster = DurableSnapshot(state).roster,
          !.compactionCause = "NoCompaction",
          !.serviceGeneration =
            DurableSnapshot(state).serviceGeneration,
          !.nextStreamEpoch =
            DurableSnapshot(state).nextStreamEpoch,
          !.serverStreamState =
            DurableSnapshot(state).serverStreams,
          !.requestGateState =
            DurableSnapshot(state).requestGates,
          !.serverClosePrefix =
            DurableSnapshot(state).serverClosePrefix,
          !.transferState = "Empty",
          !.flushState = "Empty",
          !.retryableChunk = 0,
          !.candidatePresent = FALSE,
          !.candidateSemanticallyValidated = FALSE,
          !.lifecycleCommitPhase = "Restored",
          !.pendingRolloverAuthority = "RestartRestore",
          !.validatedRestartObserved = TRUE,
          !.restartFenceAuthorized = FALSE,
          !.crashArtifactsPresent = FALSE,
          !.cleanupPerformed = TRUE,
          !.transitionAuthority = "RestartRestore",
          !.restartRequired = FALSE,
          !.failureReason = "None"]

PublishCommittedLifecycleV3ToMemory ==
  /\ RootCommittedSuccessorAheadOfMemory
  /\ state.pendingRolloverAuthority \in ChangedRosterAuthorities
  /\ ~state.restartRequired
  /\ state' =
       [state EXCEPT
          !.currentRoster = DurableSnapshot(state).roster,
          !.compactionCause = "NoCompaction",
          !.serviceGeneration =
            DurableSnapshot(state).serviceGeneration,
          !.nextStreamEpoch =
            DurableSnapshot(state).nextStreamEpoch,
          !.serverStreamState =
            DurableSnapshot(state).serverStreams,
          !.requestGateState =
            DurableSnapshot(state).requestGates,
          !.serverClosePrefix =
            DurableSnapshot(state).serverClosePrefix,
          !.recordedRetiredClosePrefix =
            IF state.pendingRolloverAuthority = "AuthenticatedTerminal"
              THEN state.serverClosePrefix
              ELSE state.recordedRetiredClosePrefix,
          !.transferState = "Empty",
          !.flushState = "Empty",
          !.retryableChunk = 0,
          !.candidatePresent = FALSE,
          !.candidateSemanticallyValidated = FALSE,
          !.lifecycleCommitPhase = "Published",
          !.receiptStage =
            IF state.receiptStage = "Retained" THEN "Consumed"
            ELSE state.receiptStage,
          !.receiptConsumeCount =
            IF state.receiptStage = "Retained" THEN 1
            ELSE state.receiptConsumeCount,
          !.successorActive = TRUE,
          !.transitionAuthority = state.pendingRolloverAuthority,
          !.pendingRolloverAuthority = "None",
          !.restartFenceAuthorized = FALSE]

ActivateRestoredLifecycleV3Successor ==
  /\ state.lifecycleCommitPhase = "Restored"
  /\ LifecycleMemoryMatchesDurableSnapshotV3
  /\ state.transitionAuthority = "RestartRestore"
  /\ ~state.successorActive
  /\ ~state.restartRequired
  /\ state' =
       [state EXCEPT
          !.lifecycleCommitPhase = "Published",
          !.pendingRolloverAuthority = "None",
          !.successorActive = TRUE]

ActivateSameRosterSuccessor ==
  /\ PredecessorTransportOwnershipOpen
  /\ state.targetRoster = state.currentRoster
  /\ state.compactionCause = "NoCompaction"
  /\ state.startupMode # "BootstrapStart"
  /\ ExactRetainedMergeSidecars
  /\ state.lifecycleCommitPhase = "Current"
  /\ ~state.candidatePresent
  /\ LifecycleJournalReady(state)
  /\ LifecycleMemoryMatchesDurableSnapshotV3
  /\ ~state.restartRequired
  /\ state' =
       [state EXCEPT
          !.receiptStage = "Consumed",
          !.receiptConsumeCount = 1,
          !.successorActive = TRUE,
          !.transitionAuthority = "SameRosterPreserved"]

RejectServiceGenerationOverflow ==
  /\ ChangedRosterReplacementNeeded
  /\ ExactRetainedMergeSidecars
  /\ state.serviceGeneration = ServiceGenerationLimit
  /\ state.lifecycleCommitPhase = "Current"
  /\ LifecycleJournalReady(state)
  /\ ~state.restartRequired
  /\ ~state.capacityRejected
  /\ state' =
       [state EXCEPT !.capacityRejected = TRUE]

FailLifecycleRootGenerationExhaustion ==
  /\ state.durableLifecycleRootV3.rootGeneration = RootGenerationLimit
  /\ state.lifecycleCommitPhase = "Current"
  /\ LifecycleJournalReady(state)
  /\ ~state.restartRequired
  /\ (ExactRetainedMergeSidecars
       \/ state.requesterEpochPhase = "Idle")
  /\ state' =
       [CrashClearedProcessLocalRolloverState(state) EXCEPT
          !.lifecycleCommitPhase = "Restarting",
          !.durableJournalValidated = FALSE,
          !.validatedRestartObserved = FALSE,
          !.restartStateDirectoryResynced = FALSE,
          !.restartRootDirectoryResynced = FALSE,
          !.crashArtifactsPresent = TRUE,
          !.cleanupPerformed = FALSE,
          !.restartRequired = TRUE,
          !.failureReason = "LifecycleRootGenerationOverflow"]

ObserveLateOldWriterCallback ==
  /\ state.successorActive
  /\ state.transitionAuthority \in ChangedRosterAuthorities
  /\ ~state.lateOldCallbackObserved
  /\ state' =
       [state EXCEPT !.lateOldCallbackObserved = TRUE]

Next ==
  \/ CreateServiceTransportOwnerPair
  \/ ValidateFinality
  \/ CloseWorkerIngress
  \/ ClearOneWorkerExactOutput
  \/ BuildImmediateSuccessor
  \/ SealAppliedHeightOutputHandoff
  \/ RejectLateExactOutputEnqueue
  \/ PresentForeignOwnerReceiptCandidate
  \/ PresentPredecessorContextMismatchCandidate
  \/ PresentPredecessorArtifactMismatchCandidate
  \/ PresentWrongImmediateSuccessorCandidate
  \/ RejectForeignOwnerReceipt
  \/ RejectPredecessorContextMismatch
  \/ RejectPredecessorArtifactMismatch
  \/ RejectWrongImmediateSuccessor
  \/ RetainExactHandoffReceipt
  \/ PublishInitialLifecycleStateSlotV3
  \/ SyncInitialLifecycleStateDirectoryV3
  \/ CrashAfterBootstrapStateReplacement
  \/ CrashAfterBootstrapStatePublication
  \/ ValidateBootstrapLifecycleCandidateV3
  \/ ValidateBootstrapLifecycleWithoutCandidateV3
  \/ ReplaceInitialLifecycleRootV3
  \/ CrashAfterBootstrapRootReplacement
  \/ CommitInitialLifecycleRootV3
  \/ ValidateRootSelectedLifecycleV3
  \/ RejectLifecycleRootShapeMismatchV3
  \/ RejectLifecycleSelectedStateMissingV3
  \/ RejectLifecycleGenerationHashMismatchV3
  \/ RejectLifecycleSemanticValidationFailureV3
  \/ ResyncValidatedLifecycleStateDirectoryV3
  \/ ResyncValidatedLifecycleRootDirectoryV3
  \/ CrashDuringValidatedRestartBeforeRootResyncV3
  \/ CleanupValidatedLifecycleArtifactsV3
  \/ CompleteBootstrapRestartWithoutCandidateV3
  \/ PersistFreshRequesterEpoch
  \/ PublishFreshRequesterEpoch
  \/ CompleteRequesterEpoch
  \/ ReopenRequesterEpochAllocator
  \/ CrashAfterRequesterEpochPersistence
  \/ RestoreRequesterEpochCounterAfterCrash
  \/ RejectRequesterEpochOverflow
  \/ RejectActiveOrdinaryRollover
  \/ RejectSameRosterFullTable
  \/ AuthenticateServerClosePrefix
  \/ TerminalizeRequestGate
  \/ TerminalizeTransfer
  \/ TerminalizeFlush
  \/ PublishSuccessorLifecycleStateSlotV3
  \/ FailSuccessorLifecycleStateSlotV3Persistence
  \/ CrashBeforeLifecycleStateSlotV3Publication
  \/ CrashAfterLifecycleStateReplacement
  \/ SyncSuccessorLifecycleStateDirectoryV3
  \/ CrashAfterLifecycleStateSlotV3Publication
  \/ FailSuccessorLifecycleRootV3Persistence
  \/ ReplaceSuccessorLifecycleRootV3
  \/ CrashAfterLifecycleRootReplacement
  \/ RecoverPredecessorLifecycleV3
  \/ CommitSuccessorLifecycleRootV3
  \/ CleanupCommittedLifecyclePredecessorV3
  \/ CrashAfterLifecycleRootV3Commit
  \/ RestoreSuccessorLifecycleV3AfterCrash
  \/ PublishCommittedLifecycleV3ToMemory
  \/ ActivateRestoredLifecycleV3Successor
  \/ ActivateSameRosterSuccessor
  \/ RejectServiceGenerationOverflow
  \/ FailLifecycleRootGenerationExhaustion
  \/ ObserveLateOldWriterCallback

TypedRolloverSpec ==
  /\ Init
  /\ [][Next]_typedRolloverVars

(***************************************************************************
The two responsive sub-specifications keep process-local exact-output
authority separate from restart authority.  The first starts from a healthy
live process and can publish only with DurableExactOutput.  The second starts
from an unvalidated restart with no process-local receipt and can publish only
after ordered validation, resynchronization, cleanup, and RestartRestore.
Neither corridor includes crash, persistence failure, requester-epoch, or
ordinary-terminalization actions.
***************************************************************************)
ResponsiveDurableExactOutputInit ==
  /\ Init
  /\ state.startupMode = "LiveProcess"
  /\ state.startupValidationFault = "None"
  /\ ChangedRosterReplacementNeeded
  /\ state.serviceGeneration < ServiceGenerationLimit
  /\ state.durableLifecycleRootV3.rootGeneration <
       RootGenerationLimit

ResponsiveDurableExactOutputNext ==
  \/ CreateServiceTransportOwnerPair
  \/ ValidateFinality
  \/ CloseWorkerIngress
  \/ ClearOneWorkerExactOutput
  \/ BuildImmediateSuccessor
  \/ SealAppliedHeightOutputHandoff
  \/ RetainExactHandoffReceipt
  \/ PublishDurableExactOutputSuccessorLifecycleStateSlotV3
  \/ SyncSuccessorLifecycleStateDirectoryV3
  \/ ReplaceSuccessorLifecycleRootV3
  \/ CommitSuccessorLifecycleRootV3
  \/ PublishCommittedLifecycleV3ToMemory

ResponsiveDurableExactOutputSpec ==
  /\ ResponsiveDurableExactOutputInit
  /\ [][ResponsiveDurableExactOutputNext]_typedRolloverVars
  /\ WF_typedRolloverVars(CreateServiceTransportOwnerPair)
  /\ WF_typedRolloverVars(ValidateFinality)
  /\ WF_typedRolloverVars(CloseWorkerIngress)
  /\ WF_typedRolloverVars(ClearOneWorkerExactOutput)
  /\ WF_typedRolloverVars(BuildImmediateSuccessor)
  /\ WF_typedRolloverVars(SealAppliedHeightOutputHandoff)
  /\ WF_typedRolloverVars(RetainExactHandoffReceipt)
  /\ WF_typedRolloverVars(
       PublishDurableExactOutputSuccessorLifecycleStateSlotV3)
  /\ WF_typedRolloverVars(
       SyncSuccessorLifecycleStateDirectoryV3)
  /\ WF_typedRolloverVars(ReplaceSuccessorLifecycleRootV3)
  /\ WF_typedRolloverVars(CommitSuccessorLifecycleRootV3)
  /\ WF_typedRolloverVars(PublishCommittedLifecycleV3ToMemory)

ResponsiveRestartRestoreInit ==
  /\ Init
  /\ state.startupMode = "UnvalidatedRestart"
  /\ state.startupValidationFault = "None"
  /\ ChangedRosterReplacementNeeded
  /\ state.serviceGeneration < ServiceGenerationLimit
  /\ state.durableLifecycleRootV3.rootGeneration <
       RootGenerationLimit

ResponsiveRestartRestoreNext ==
  \/ ValidateRootSelectedLifecycleV3
  \/ ResyncValidatedLifecycleStateDirectoryV3
  \/ ResyncValidatedLifecycleRootDirectoryV3
  \/ CleanupValidatedLifecycleArtifactsV3
  \/ RecoverPredecessorLifecycleV3
  \/ PublishRestartRestoreSuccessorLifecycleStateSlotV3
  \/ SyncSuccessorLifecycleStateDirectoryV3
  \/ ReplaceSuccessorLifecycleRootV3
  \/ CommitSuccessorLifecycleRootV3
  \/ PublishCommittedLifecycleV3ToMemory

ResponsiveRestartRestoreSpec ==
  /\ ResponsiveRestartRestoreInit
  /\ [][ResponsiveRestartRestoreNext]_typedRolloverVars
  /\ WF_typedRolloverVars(ValidateRootSelectedLifecycleV3)
  /\ WF_typedRolloverVars(
       ResyncValidatedLifecycleStateDirectoryV3)
  /\ WF_typedRolloverVars(
       ResyncValidatedLifecycleRootDirectoryV3)
  /\ WF_typedRolloverVars(
       CleanupValidatedLifecycleArtifactsV3)
  /\ WF_typedRolloverVars(RecoverPredecessorLifecycleV3)
  /\ WF_typedRolloverVars(
       PublishRestartRestoreSuccessorLifecycleStateSlotV3)
  /\ WF_typedRolloverVars(
       SyncSuccessorLifecycleStateDirectoryV3)
  /\ WF_typedRolloverVars(ReplaceSuccessorLifecycleRootV3)
  /\ WF_typedRolloverVars(CommitSuccessorLifecycleRootV3)
  /\ WF_typedRolloverVars(PublishCommittedLifecycleV3ToMemory)

TypedRolloverTypeInvariant ==
  state \in
    [startupMode: StartupModes,
     startupValidationFault: LifecycleValidationFaults,
     startupValidationRejected: BOOLEAN,
     validationFailureObserved: BOOLEAN,
     targetRoster: Rosters,
     compactionCause: CompactionCauses,
     currentRoster: Rosters,
     baselineRoster: Rosters,
     baselineServiceGeneration:
       InitialServiceGeneration..ServiceGenerationLimit,
     finalityValidated: BOOLEAN,
     workerIngressClosed: BOOLEAN,
     workerOutstanding: 0..InitialWorkerOutstanding,
     ownerSealed: BOOLEAN,
     serviceOwnerNonce: OwnerNonces \cup {NoIdentity},
     transportOwnerNonce: OwnerNonces \cup {NoIdentity},
     constructionParent: Parents \cup {NoIdentity},
     constructionSuccessor: Successors \cup {NoIdentity},
     presentedHandoffCandidate: HandoffCandidateKinds,
     receiptStage: ReceiptStages,
     receiptOwnerNonce: OwnerNonces \cup {NoIdentity},
     receiptContext: Contexts \cup {NoIdentity},
     receiptArtifact: Artifacts \cup {NoIdentity},
     retainedSuccessor: Successors \cup {NoIdentity},
     receiptConsumeCount: 0..1,
     serviceGeneration: InitialServiceGeneration..ServiceGenerationLimit,
     nextStreamEpoch: InitialNextStreamEpoch..StreamEpochLimit,
     requesterEpochPhase: RequesterEpochPhases,
     pendingStreamEpoch: 0..StreamEpochLimit,
     activeStreamEpoch: 0..StreamEpochLimit,
     lastCompletedStreamEpoch: 0..StreamEpochLimit,
     requesterEpochReplacementRestored: BOOLEAN,
     serverStreamState: LifecycleEntryStates,
     requestGateState: LifecycleEntryStates,
     serverClosePrefix: 0..HighestSemanticSequence,
     authenticatedCloseHistory: 0..HighestSemanticSequence,
     recordedRetiredClosePrefix: 0..HighestSemanticSequence,
     transferState: LifecycleEntryStates,
     flushState: LifecycleEntryStates,
     durableLifecycleRootV3: LifecycleRootV3Set,
     durableLifecycleStateSlotsV3: LifecycleStateSlotsV3Set,
     syncedLifecycleRootV3: LifecycleRootV3Set,
     syncedLifecycleStateSlotsV3: LifecycleStateSlotsV3Set,
     candidateLifecycleSnapshotV3: LifecycleSnapshotV3Set,
     candidatePresent: BOOLEAN,
     candidateStateSlot: StateSlots,
     candidateSemanticallyValidated: BOOLEAN,
     lifecycleCommitPhase: LifecycleCommitPhases,
     pendingRolloverAuthority: PendingRolloverAuthorities,
     durableJournalValidated: BOOLEAN,
     validatedRestartObserved: BOOLEAN,
     restartFenceAuthorized: BOOLEAN,
     crashArtifactsPresent: BOOLEAN,
     cleanupPerformed: BOOLEAN,
     restartStateDirectoryResynced: BOOLEAN,
     restartRootDirectoryResynced: BOOLEAN,
     retryableChunk: 0..1,
     successorActive: BOOLEAN,
     transitionAuthority: TransitionAuthorities,
     capacityRejected: BOOLEAN,
     restartRequired: BOOLEAN,
     failureReason: FailureReasons,
     lateEnqueueRejected: BOOLEAN,
     foreignReceiptRejected: BOOLEAN,
     predecessorMismatchRejected: BOOLEAN,
     wrongSuccessorRejected: BOOLEAN,
     lateOldCallbackObserved: BOOLEAN,
     bootstrapCrashObserved: BOOLEAN,
     stateSlotCrashObserved: BOOLEAN,
     rootCrashObserved: BOOLEAN,
     epochCrashObserved: BOOLEAN,
     secondCrashObserved: BOOLEAN]

CompactionGeometryInvariant ==
  CompactionCauseMatchesGeometry

ReceiptLifecycleInvariant ==
  /\ (state.receiptStage \in {"Absent", "Lost"} =>
        /\ state.receiptOwnerNonce = NoIdentity
        /\ state.receiptContext = NoIdentity
        /\ state.receiptArtifact = NoIdentity
        /\ state.retainedSuccessor = NoIdentity)
  /\ (state.receiptStage \in {"Minted", "Retained", "Consumed"} =>
        /\ state.ownerSealed
        /\ ExactPredecessorReceipt)
  /\ (state.receiptStage \in {"Retained", "Consumed"} =>
        /\ ExactSuccessorConstruction
        /\ state.retainedSuccessor = ExpectedSuccessor)
  /\ (state.receiptStage = "Consumed" =>
        state.receiptConsumeCount = 1)
  /\ (state.receiptStage = "Lost" =>
        state.receiptConsumeCount = 0)

ExactServiceTransportOwnerPairInvariant ==
  \/ /\ state.serviceOwnerNonce = NoIdentity
     /\ state.transportOwnerNonce = NoIdentity
  \/ ExactServiceTransportOwnerPair

UnconsumedPredecessorTransportOwnershipInvariant ==
  state.receiptStage \in {"Minted", "Retained"} =>
    PredecessorTransportOwnershipOpen

FinalSealRejectsLateEnqueueInvariant ==
  /\ (state.ownerSealed => FinalExactOutputSeal)
  /\ (state.lateEnqueueRejected =>
        /\ state.restartRequired
        /\ ~state.successorActive
        /\ state.failureReason = "LateExactOutputEnqueue")

MismatchRejectionInvariant ==
  /\ (state.foreignReceiptRejected =>
        /\ state.restartRequired
        /\ state.failureReason = "ForeignOwnerMismatch")
  /\ (state.predecessorMismatchRejected =>
        /\ state.restartRequired
        /\ state.failureReason \in
             {"PredecessorContextMismatch",
              "PredecessorArtifactMismatch"})
  /\ (state.wrongSuccessorRejected =>
        /\ state.restartRequired
        /\ state.failureReason = "ImmediateSuccessorMismatch")

FailureLatchInvariant ==
  /\ (state.restartRequired => ~state.successorActive)
  /\ (~state.restartRequired => state.failureReason = "None")

RootAnchoredLifecycleV3Invariant ==
  /\ state.durableLifecycleRootV3 \in LifecycleRootV3Set
  /\ state.syncedLifecycleRootV3 \in LifecycleRootV3Set
  /\ state.durableLifecycleRootV3.version = 3
  /\ state.syncedLifecycleRootV3.version = 3
  /\ LifecycleRootShapeIsValid(state.durableLifecycleRootV3)
  /\ LifecycleRootShapeIsValid(state.syncedLifecycleRootV3)
  /\ state.durableLifecycleStateSlotsV3 \in
       LifecycleStateSlotsV3Set
  /\ state.syncedLifecycleStateSlotsV3 \in
       LifecycleStateSlotsV3Set
  /\ state.syncedLifecycleRootV3.rootGeneration <=
       state.durableLifecycleRootV3.rootGeneration
  /\ state.durableLifecycleRootV3.rootGeneration <=
       state.syncedLifecycleRootV3.rootGeneration + 1
  /\ (state.durableLifecycleRootV3.shape = "Committed" =>
        /\ RootSelectedLifecyclePairMatches(state)
        /\ LifecycleSnapshotSemanticallyValid(DurableSnapshot(state))
        /\ state.durableLifecycleRootV3.rootGeneration =
             DurableSnapshot(state).rootGeneration
        /\ state.serviceGeneration <=
             DurableSnapshot(state).serviceGeneration
        /\ state.nextStreamEpoch <=
             DurableSnapshot(state).nextStreamEpoch)
  /\ (state.syncedLifecycleRootV3.shape = "Committed" =>
        /\ SyncedRootSelectedLifecyclePairMatches(state)
        /\ LifecycleSnapshotSemanticallyValid(
             SyncedLifecycleSnapshot(state)))
  /\ (state.durableLifecycleRootV3.shape = "Bootstrap" =>
        state.lifecycleCommitPhase \in
          {"Bootstrap",
           "BootstrapStateReplaced",
           "BootstrapStatePublished",
           "Restarting"})
  /\ (~LifecycleStateDirectoryIsSynced(state) =>
        state.lifecycleCommitPhase \in
          {"BootstrapStateReplaced",
           "BootstrapStatePublished",
           "StateSlotReplaced",
           "Restarting"})
  /\ (~LifecycleRootDirectoryIsSynced(state) =>
        state.lifecycleCommitPhase \in
          {"BootstrapRootReplaced",
           "RootReplaced",
           "Restarting"})
  /\ (state.candidatePresent =>
        /\ state.candidateLifecycleSnapshotV3.version = 3
        /\ state.candidateStateSlot \in StateSlots
        /\ state.durableLifecycleStateSlotsV3[
             state.candidateStateSlot] =
             state.candidateLifecycleSnapshotV3)

SemanticValidationBeforeCleanupInvariant ==
  /\ (state.cleanupPerformed =>
        /\ state.durableJournalValidated
        /\ LifecycleStateDirectoryIsSynced(state)
        /\ LifecycleRootDirectoryIsSynced(state))
  /\ (~state.crashArtifactsPresent /\ state.cleanupPerformed =>
        \/ /\ state.durableLifecycleRootV3.shape = "Bootstrap"
           /\ state.durableLifecycleStateSlotsV3[
                LifecycleStateSlot(InitialRootGeneration)] =
                NoLifecycleSnapshot
        \/ /\ state.durableLifecycleRootV3.shape = "Committed"
           /\ RootSelectedLifecyclePairMatches(state)
           /\ LifecycleSnapshotSemanticallyValid(
                DurableSnapshot(state)))
  /\ (~state.durableJournalValidated =>
        ~state.cleanupPerformed)
  /\ (state.cleanupPerformed /\ state.restartRequired =>
        /\ state.restartStateDirectoryResynced
        /\ state.restartRootDirectoryResynced)

ValidatedCleanupRemovesInactiveSlotInvariant ==
  (/\ state.cleanupPerformed
   /\ ~state.crashArtifactsPresent
   /\ state.durableLifecycleRootV3.shape = "Committed")
    =>
      /\ state.durableLifecycleStateSlotsV3[
           InactiveLifecycleStateSlot(state)] =
           NoLifecycleSnapshot
      /\ state.syncedLifecycleStateSlotsV3[
           InactiveLifecycleStateSlot(state)] =
           NoLifecycleSnapshot

InvalidLifecycleStartupFailsClosedInvariant ==
  /\ (state.startupValidationFault # "None" =>
        /\ state.startupMode = "UnvalidatedRestart"
        /\ state.restartRequired
        /\ ~state.durableJournalValidated
        /\ state.crashArtifactsPresent
        /\ ~state.cleanupPerformed
        /\ ~state.successorActive)
  /\ (state.startupValidationRejected =>
        /\ state.validationFailureObserved
        /\ state.failureReason =
             state.startupValidationFault
        /\ state.crashArtifactsPresent
        /\ ~state.cleanupPerformed)

ProcessLocalAuthorityAfterCrashInvariant ==
  (/\ state.restartRequired
   /\ ~state.durableJournalValidated)
    =>
      /\ ~state.finalityValidated
      /\ ~state.workerIngressClosed
      /\ state.workerOutstanding = InitialWorkerOutstanding
      /\ ~state.ownerSealed
      /\ state.serviceOwnerNonce = NoIdentity
      /\ state.transportOwnerNonce = NoIdentity
      /\ state.constructionParent = NoIdentity
      /\ state.constructionSuccessor = NoIdentity
      /\ state.presentedHandoffCandidate = "None"
      /\ state.receiptStage \in {"Absent", "Lost"}
      /\ state.receiptOwnerNonce = NoIdentity
      /\ state.receiptContext = NoIdentity
      /\ state.receiptArtifact = NoIdentity
      /\ state.retainedSuccessor = NoIdentity
      /\ ~state.candidatePresent
      /\ state.pendingRolloverAuthority = "None"
      /\ state.transitionAuthority = "None"
      /\ ~state.restartFenceAuthorized

LifecycleCommitPhaseInvariant ==
  CASE state.lifecycleCommitPhase = "Bootstrap" ->
         /\ state.durableLifecycleRootV3 =
              BootstrapLifecycleRootV3
         /\ ~state.candidatePresent
    [] state.lifecycleCommitPhase = "BootstrapStateReplaced" ->
         /\ state.durableLifecycleRootV3 =
              BootstrapLifecycleRootV3
         /\ state.candidatePresent
         /\ ~LifecycleStateDirectoryIsSynced(state)
    [] state.lifecycleCommitPhase = "BootstrapStatePublished" ->
         /\ state.durableLifecycleRootV3 =
              BootstrapLifecycleRootV3
         /\ state.candidatePresent
         /\ state.candidateStateSlot =
              LifecycleStateSlot(InitialRootGeneration)
         /\ state.candidateLifecycleSnapshotV3 =
              InitialLifecycleSnapshotV3(state.targetRoster)
         /\ (state.restartRequired
               \/ LifecycleStateDirectoryIsSynced(state))
    [] state.lifecycleCommitPhase = "BootstrapRootReplaced" ->
         /\ state.durableLifecycleRootV3.shape = "Committed"
         /\ state.syncedLifecycleRootV3 =
              BootstrapLifecycleRootV3
         /\ DurableSnapshot(state) =
              InitialLifecycleSnapshotV3(state.targetRoster)
         /\ state.candidatePresent
         /\ ~LifecycleRootDirectoryIsSynced(state)
    [] state.lifecycleCommitPhase = "Current" ->
         /\ ~state.candidatePresent
         /\ state.durableLifecycleRootV3.shape = "Committed"
         /\ LifecycleTablesMatchDurableSnapshotV3
         /\ (state.nextStreamEpoch =
               DurableSnapshot(state).nextStreamEpoch
             \/ RequesterEpochPersistenceAheadOfMemory)
    [] state.lifecycleCommitPhase = "StateSlotReplaced" ->
         /\ DurableCandidateStateSlotAheadOfRoot
         /\ LifecycleMemoryMatchesDurableSnapshotV3
         /\ ~LifecycleStateDirectoryIsSynced(state)
    [] state.lifecycleCommitPhase = "StateSlotPublished" ->
         /\ DurableCandidateStateSlotAheadOfRoot
         /\ LifecycleMemoryMatchesDurableSnapshotV3
         /\ LifecycleStateDirectoryIsSynced(state)
         /\ state.candidateSemanticallyValidated
         /\ state.pendingRolloverAuthority \in
              ChangedRosterAuthorities
    [] state.lifecycleCommitPhase = "RootReplaced" ->
         /\ state.candidatePresent
         /\ RootSelectedSuccessorAheadOfMemory
         /\ LifecycleStateDirectoryIsSynced(state)
         /\ ~LifecycleRootDirectoryIsSynced(state)
    [] state.lifecycleCommitPhase = "RootCommitted" ->
         /\ RootCommittedSuccessorAheadOfMemory
         /\ state.pendingRolloverAuthority \in
              ChangedRosterAuthorities
    [] state.lifecycleCommitPhase = "Restarting" ->
         /\ state.restartRequired
         /\ ~state.candidatePresent
    [] state.lifecycleCommitPhase = "Restored" ->
         /\ ~state.candidatePresent
         /\ LifecycleMemoryMatchesDurableSnapshotV3
         /\ state.serverStreamState = "Empty"
         /\ state.requestGateState = "Empty"
         /\ state.transferState = "Empty"
         /\ state.flushState = "Empty"
    [] state.lifecycleCommitPhase = "Published" ->
         /\ ~state.candidatePresent
         /\ LifecycleMemoryMatchesDurableSnapshotV3
         /\ state.serverStreamState = "Empty"
         /\ state.requestGateState = "Empty"
         /\ state.transferState = "Empty"
         /\ state.flushState = "Empty"

AuthorityGatedGenerationAdvanceInvariant ==
  /\ (state.pendingRolloverAuthority = "AuthenticatedTerminal" =>
        AllOldLifecycleTerminal)
  /\ (state.pendingRolloverAuthority = "DurableExactOutput" =>
        /\ state.durableJournalValidated
        /\ ExactRetainedMergeSidecars)
  /\ (state.pendingRolloverAuthority = "RestartRestore" =>
        /\ state.durableJournalValidated
        /\ state.validatedRestartObserved
        /\ state.restartFenceAuthorized
        /\ state.receiptStage \in {"Absent", "Lost"})
  /\ (state.restartFenceAuthorized =>
        /\ state.durableJournalValidated
        /\ state.validatedRestartObserved
        /\ state.cleanupPerformed
        /\ state.restartStateDirectoryResynced
        /\ state.restartRootDirectoryResynced
        /\ state.targetRoster # state.currentRoster
        /\ state.receiptStage \in {"Absent", "Lost"})
  /\ (state.durableLifecycleRootV3.shape = "Committed"
       /\ DurableSnapshot(state).serviceGeneration >
            state.serviceGeneration =>
        /\ ChangedRosterReplacementNeeded
        /\ DurableSnapshot(state).roster = state.targetRoster
        /\ DurableSnapshot(state).serverStreams = "Empty"
        /\ DurableSnapshot(state).requestGates = "Empty"
        /\ DurableSnapshot(state).serverClosePrefix = 0)
  /\ (state.currentRoster # state.baselineRoster =>
        /\ state.serviceGeneration >
             state.baselineServiceGeneration
        /\ state.currentRoster = state.targetRoster
        /\ state.serverStreamState = "Empty"
        /\ state.requestGateState = "Empty"
        /\ state.transferState = "Empty"
        /\ state.flushState = "Empty"
        /\ state.transitionAuthority \in ChangedRosterAuthorities)
  /\ (state.transitionAuthority = "AuthenticatedTerminal" =>
        /\ state.authenticatedCloseHistory =
             HighestSemanticSequence
        /\ state.recordedRetiredClosePrefix =
             HighestSemanticSequence)
  /\ (state.transitionAuthority = "DurableExactOutput" =>
        /\ state.durableJournalValidated
        /\ state.receiptStage = "Consumed")
  /\ (state.transitionAuthority = "RestartRestore" =>
        /\ state.durableJournalValidated
        /\ state.validatedRestartObserved
        /\ state.receiptStage \in {"Absent", "Lost"})

SameRosterTransportPreservationInvariant ==
  (/\ state.targetRoster = state.baselineRoster
   /\ state.currentRoster = state.baselineRoster
   /\ state.durableLifecycleRootV3.shape = "Committed"
   /\ ~state.restartRequired) =>
    /\ state.serviceGeneration = state.baselineServiceGeneration
    /\ DurableSnapshot(state).serviceGeneration =
         state.baselineServiceGeneration
    /\ (IF state.startupMode = "BootstrapStart"
          THEN /\ state.serverStreamState = "Empty"
               /\ state.requestGateState = "Empty"
               /\ state.transferState = "Empty"
               /\ state.flushState = "Empty"
               /\ state.retryableChunk = 0
          ELSE /\ state.serverStreamState = "Active"
               /\ state.requestGateState = "Active"
               /\ state.transferState = "Active"
               /\ state.flushState = "Active"
               /\ state.retryableChunk = 1)
    /\ ~state.candidatePresent
    /\ state.lifecycleCommitPhase = "Current"
    /\ state.transitionAuthority \in
         {"None", "SameRosterPreserved"}

RequesterEpochInvariant ==
  /\ state.lastCompletedStreamEpoch <= state.nextStreamEpoch
  /\ (state.pendingStreamEpoch # 0 =>
        /\ state.requesterEpochPhase = "Persisted"
        /\ state.pendingStreamEpoch = state.nextStreamEpoch + 1
        /\ state.pendingStreamEpoch >
             state.lastCompletedStreamEpoch
        /\ DurableSnapshot(state).nextStreamEpoch =
             state.pendingStreamEpoch
        /\ DurableSnapshot(state).requesterStreamEpoch =
             state.pendingStreamEpoch)
  /\ (state.activeStreamEpoch # 0 =>
        /\ state.requesterEpochPhase = "InUse"
        /\ state.activeStreamEpoch > state.lastCompletedStreamEpoch
        /\ state.activeStreamEpoch <= state.nextStreamEpoch)
  /\ (state.requesterEpochReplacementRestored =>
        /\ state.epochCrashObserved
        /\ state.nextStreamEpoch =
             DurableSnapshot(state).nextStreamEpoch
        /\ (state.activeStreamEpoch =
              DurableSnapshot(state).requesterStreamEpoch
             \/ state.lastCompletedStreamEpoch >=
                  DurableSnapshot(state).requesterStreamEpoch))

CrashRecoveryInvariant ==
  /\ (state.bootstrapCrashObserved =>
        state.validatedRestartObserved
          \/ state.restartRequired)
  /\ (state.stateSlotCrashObserved =>
        state.validatedRestartObserved
          \/ state.restartRequired)
  /\ (state.rootCrashObserved =>
        state.validatedRestartObserved
          \/ state.restartRequired)
  /\ (state.epochCrashObserved /\ ~state.restartRequired =>
        state.requesterEpochReplacementRestored)
  /\ (state.secondCrashObserved =>
        state.validatedRestartObserved
          \/ state.restartRequired)
  /\ (state.failureReason \in
        {"CrashAfterBootstrapStateReplacement",
         "CrashAfterBootstrapStatePublication",
         "CrashAfterBootstrapRootReplacement"} =>
        state.bootstrapCrashObserved)
  /\ (state.failureReason =
        "CrashAfterLifecycleStateSlotV3Publication" =>
        /\ state.stateSlotCrashObserved
        /\ ~state.candidatePresent)
  /\ (state.failureReason = "CrashAfterLifecycleRootV3Commit" =>
        /\ state.rootCrashObserved
        /\ ~state.candidatePresent)
  /\ (state.failureReason =
        "CrashAfterRequesterEpochPersistence" =>
        /\ state.epochCrashObserved
        /\ state.requesterEpochPhase = "Restarting"
        /\ state.activeStreamEpoch = 0)

NoForgedAuthenticatedClosePrefixInvariant ==
  /\ state.serverClosePrefix <= state.authenticatedCloseHistory
  /\ state.recordedRetiredClosePrefix <=
       state.authenticatedCloseHistory
  /\ (state.serverStreamState = "Terminal" =>
        state.serverClosePrefix = HighestSemanticSequence)
  /\ (state.candidatePresent =>
        state.candidateLifecycleSnapshotV3.serverClosePrefix = 0)
  /\ (state.durableLifecycleRootV3.shape = "Committed"
       /\ DurableSnapshot(state).serviceGeneration >
            state.serviceGeneration =>
        DurableSnapshot(state).serverClosePrefix = 0)

LateOldCallbackIsolationInvariant ==
  state.lateOldCallbackObserved =>
    /\ state.successorActive
    /\ state.transitionAuthority \in ChangedRosterAuthorities
    /\ state.lifecycleCommitPhase = "Published"
    /\ state.serverStreamState = "Empty"
    /\ state.requestGateState = "Empty"

TypedRolloverSafetyInvariant ==
  /\ TypedRolloverTypeInvariant
  /\ CompactionGeometryInvariant
  /\ ExactServiceTransportOwnerPairInvariant
  /\ UnconsumedPredecessorTransportOwnershipInvariant
  /\ ReceiptLifecycleInvariant
  /\ FinalSealRejectsLateEnqueueInvariant
  /\ MismatchRejectionInvariant
  /\ FailureLatchInvariant
  /\ RootAnchoredLifecycleV3Invariant
  /\ SemanticValidationBeforeCleanupInvariant
  /\ ValidatedCleanupRemovesInactiveSlotInvariant
  /\ InvalidLifecycleStartupFailsClosedInvariant
  /\ ProcessLocalAuthorityAfterCrashInvariant
  /\ LifecycleCommitPhaseInvariant
  /\ AuthorityGatedGenerationAdvanceInvariant
  /\ SameRosterTransportPreservationInvariant
  /\ RequesterEpochInvariant
  /\ CrashRecoveryInvariant
  /\ NoForgedAuthenticatedClosePrefixInvariant
  /\ LateOldCallbackIsolationInvariant

CapacityRejectionStepSafety ==
  (/\ ~state.capacityRejected
   /\ state'.capacityRejected)
    =>
      /\ LifecycleMemory(state') = LifecycleMemory(state)
      /\ DurableLifecycle(state') = DurableLifecycle(state)
      /\ CandidateLifecycle(state') = CandidateLifecycle(state)

StateSlotBeforeRootCommitStepSafety ==
  (/\ ~state.candidatePresent
   /\ state'.candidatePresent)
    =>
      /\ state'.candidateLifecycleSnapshotV3.rootGeneration =
           state.durableLifecycleRootV3.rootGeneration + 1
      /\ state'.candidateStateSlot =
           LifecycleStateSlot(
             state'.candidateLifecycleSnapshotV3.rootGeneration)
      /\ state'.candidateStateSlot #
           SelectedLifecycleStateSlot(state)
      /\ state'.candidateStateSlot =
           1 - SelectedLifecycleStateSlot(state)
      /\ state'.durableLifecycleRootV3 =
           state.durableLifecycleRootV3
      /\ state'.syncedLifecycleRootV3 =
           state.syncedLifecycleRootV3
      /\ state'.durableLifecycleStateSlotsV3[
           state'.candidateStateSlot] =
           state'.candidateLifecycleSnapshotV3
      /\ \A slot \in StateSlots \ {state'.candidateStateSlot}:
           state'.durableLifecycleStateSlotsV3[slot] =
             state.durableLifecycleStateSlotsV3[slot]
      /\ SelectedLifecycleSnapshotV3(state') =
           SelectedLifecycleSnapshotV3(state)
      /\ LifecycleMemory(state') = LifecycleMemory(state)

RootCommitBeforeMemoryPublicationStepSafety ==
  (/\ state.syncedLifecycleRootV3.shape = "Committed"
   /\ SyncedLifecycleSnapshot(state').serviceGeneration >
        SyncedLifecycleSnapshot(state).serviceGeneration)
    =>
      /\ state.lifecycleCommitPhase \in
           {"RootReplaced", "Restarting"}
      /\ (state.lifecycleCommitPhase = "RootReplaced" =>
            state.candidatePresent)
      /\ (state.lifecycleCommitPhase = "Restarting" =>
            /\ state.restartRequired
            /\ state.durableJournalValidated
            /\ state.restartStateDirectoryResynced)
      /\ state'.syncedLifecycleRootV3 =
           state.durableLifecycleRootV3
      /\ SyncedSelectedLifecycleStateSlot(state') =
           SelectedLifecycleStateSlot(state)
      /\ state'.durableLifecycleStateSlotsV3 =
           state.durableLifecycleStateSlotsV3
      /\ state'.syncedLifecycleStateSlotsV3 =
           state.syncedLifecycleStateSlotsV3
      /\ LifecycleStateDirectoryIsSynced(state)
      /\ LifecycleMemory(state') = LifecycleMemory(state)

RootGenerationMonotonicStepSafety ==
  /\ state'.syncedLifecycleRootV3.rootGeneration >=
       state.syncedLifecycleRootV3.rootGeneration
  /\ (state'.syncedLifecycleRootV3.rootGeneration =
        state.syncedLifecycleRootV3.rootGeneration =>
        /\ state'.syncedLifecycleRootV3 =
             state.syncedLifecycleRootV3
        /\ SyncedSelectedLifecycleSnapshotV3(state') =
             SyncedSelectedLifecycleSnapshotV3(state))
  /\ (state'.syncedLifecycleRootV3.rootGeneration >
        state.syncedLifecycleRootV3.rootGeneration =>
        /\ state'.syncedLifecycleRootV3.rootGeneration =
             state.syncedLifecycleRootV3.rootGeneration + 1
        /\ state'.syncedLifecycleRootV3.shape = "Committed"
        /\ SyncedSelectedLifecycleStateSlot(state') =
             LifecycleStateSlot(
               state'.syncedLifecycleRootV3.rootGeneration)
        /\ SyncedSelectedLifecycleStateSlot(state') #
             SyncedSelectedLifecycleStateSlot(state)
        /\ SyncedSelectedLifecycleStateSlot(state') =
             1 - SyncedSelectedLifecycleStateSlot(state)
        /\ state'.syncedLifecycleRootV3.snapshotDigest =
             LifecycleSnapshotDigest(
               SyncedSelectedLifecycleSnapshotV3(state')))

LifecycleV3PublicationStepSafety ==
  state'.serviceGeneration > state.serviceGeneration
    =>
      /\ DurableSnapshot(state).serviceGeneration =
           state'.serviceGeneration
      /\ DurableSnapshot(state).serverStreams = "Empty"
      /\ DurableSnapshot(state).requestGates = "Empty"
      /\ DurableSnapshot(state).serverClosePrefix = 0
      /\ state'.serverStreamState = "Empty"
      /\ state'.requestGateState = "Empty"
      /\ state'.transferState = "Empty"
      /\ state'.flushState = "Empty"

ForcedFenceDoesNotForgeCloseStepSafety ==
  (/\ state'.serviceGeneration > state.serviceGeneration
   /\ state'.transitionAuthority \in
        {"DurableExactOutput", "RestartRestore"})
    =>
      state'.recordedRetiredClosePrefix =
        state.recordedRetiredClosePrefix

RequesterEpochUseStepSafety ==
  (/\ state.activeStreamEpoch = 0
   /\ state'.activeStreamEpoch # 0)
    =>
      /\ (state.requesterEpochPhase = "Persisted"
           \/ /\ state.restartRequired
              /\ state.durableJournalValidated
              /\ state.cleanupPerformed)
      /\ DurableSnapshot(state).nextStreamEpoch =
           state'.activeStreamEpoch
      /\ DurableSnapshot(state).requesterStreamEpoch =
           state'.activeStreamEpoch
      /\ state'.nextStreamEpoch =
           DurableSnapshot(state).nextStreamEpoch

CrashDropsProcessLocalAuthorityStepSafety ==
  (/\ state.durableJournalValidated
   /\ ~state'.durableJournalValidated
   /\ state'.restartRequired
   /\ state'.crashArtifactsPresent)
    =>
      /\ ~state'.finalityValidated
      /\ ~state'.workerIngressClosed
      /\ state'.workerOutstanding = InitialWorkerOutstanding
      /\ ~state'.ownerSealed
      /\ state'.serviceOwnerNonce = NoIdentity
      /\ state'.transportOwnerNonce = NoIdentity
      /\ state'.constructionParent = NoIdentity
      /\ state'.constructionSuccessor = NoIdentity
      /\ state'.presentedHandoffCandidate = "None"
      /\ state'.receiptStage \in {"Absent", "Lost"}
      /\ state'.receiptOwnerNonce = NoIdentity
      /\ state'.receiptContext = NoIdentity
      /\ state'.receiptArtifact = NoIdentity
      /\ state'.retainedSuccessor = NoIdentity
      /\ ~state'.candidatePresent
      /\ state'.pendingRolloverAuthority = "None"
      /\ state'.transitionAuthority = "None"
      /\ ~state'.restartFenceAuthorized

ValidationFailurePreservesArtifactsStepSafety ==
  (/\ ~state.validationFailureObserved
   /\ state'.validationFailureObserved)
    =>
      /\ DurableLifecycle(state') = DurableLifecycle(state)
      /\ CandidateLifecycle(state') = CandidateLifecycle(state)
      /\ state'.crashArtifactsPresent
      /\ ~state'.cleanupPerformed
      /\ ~state'.durableJournalValidated
      /\ ~state'.successorActive

ValidatedCleanupStepSafety ==
  (/\ state.restartRequired
   /\ ~state.cleanupPerformed
   /\ state'.cleanupPerformed
   /\ state.durableLifecycleRootV3.shape = "Committed")
    =>
      /\ state.restartStateDirectoryResynced
      /\ state.restartRootDirectoryResynced
      /\ state'.durableLifecycleStateSlotsV3[
           InactiveLifecycleStateSlot(state')] =
           NoLifecycleSnapshot
      /\ state'.syncedLifecycleStateSlotsV3[
           InactiveLifecycleStateSlot(state')] =
           NoLifecycleSnapshot

RequesterIncarnationRestoreStepSafety ==
  (/\ state.failureReason =
        "CrashAfterRequesterEpochPersistence"
   /\ ~state.requesterEpochReplacementRestored
   /\ state'.requesterEpochReplacementRestored)
    =>
      /\ state'.nextStreamEpoch =
           DurableSnapshot(state).nextStreamEpoch
      /\ state'.activeStreamEpoch =
           DurableSnapshot(state).requesterStreamEpoch
      /\ state'.activeStreamEpoch =
           state'.nextStreamEpoch

CapacityRejectionActionProperty ==
  [][CapacityRejectionStepSafety]_typedRolloverVars

StateSlotBeforeRootCommitActionProperty ==
  [][StateSlotBeforeRootCommitStepSafety]_typedRolloverVars

RootCommitBeforeMemoryPublicationActionProperty ==
  [][RootCommitBeforeMemoryPublicationStepSafety]_typedRolloverVars

RootGenerationMonotonicActionProperty ==
  [][RootGenerationMonotonicStepSafety]_typedRolloverVars

LifecycleV3PublicationActionProperty ==
  [][LifecycleV3PublicationStepSafety]_typedRolloverVars

ForcedFenceDoesNotForgeCloseActionProperty ==
  [][ForcedFenceDoesNotForgeCloseStepSafety]_typedRolloverVars

RequesterEpochUseActionProperty ==
  [][RequesterEpochUseStepSafety]_typedRolloverVars

CrashDropsProcessLocalAuthorityActionProperty ==
  [][CrashDropsProcessLocalAuthorityStepSafety]_typedRolloverVars

ValidationFailurePreservesArtifactsActionProperty ==
  [][ValidationFailurePreservesArtifactsStepSafety]_typedRolloverVars

ValidatedCleanupActionProperty ==
  [][ValidatedCleanupStepSafety]_typedRolloverVars

RequesterIncarnationRestoreActionProperty ==
  [][RequesterIncarnationRestoreStepSafety]_typedRolloverVars

NoRolloverFailure ==
  /\ ~state.restartRequired
  /\ state.failureReason = "None"

DurableExactOutputSuccessorActiveWithoutRestart ==
  /\ state.currentRoster = state.targetRoster
  /\ state.successorActive
  /\ ~state.restartRequired
  /\ state.transitionAuthority = "DurableExactOutput"

RestartRestoreSuccessorActiveWithoutRestart ==
  /\ state.currentRoster = state.targetRoster
  /\ state.successorActive
  /\ ~state.restartRequired
  /\ state.transitionAuthority = "RestartRestore"

ResponsiveDurableExactOutputRolloverLiveness ==
  ResponsiveDurableExactOutputSpec =>
    (state.finalityValidated
      ~> DurableExactOutputSuccessorActiveWithoutRestart)

ResponsiveRestartRestoreRolloverLiveness ==
  ResponsiveRestartRestoreSpec =>
    (state.restartRequired
      ~> RestartRestoreSuccessorActiveWithoutRestart)

=============================================================================
