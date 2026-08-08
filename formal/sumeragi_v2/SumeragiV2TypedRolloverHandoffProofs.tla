---- MODULE SumeragiV2TypedRolloverHandoffProofs ----
EXTENDS SumeragiV2TypedRolloverHandoff, TLAPS

(***************************************************************************
Deductive obligations for the authority-gated, root-anchored V3 typed
rollover model.

The declarations separate move-only durable exact-output authority from
validated restart authority.  They also expose both filesystem layers used by
the model: replacement writes visible to the process and parent-directory
synchronization.  A crash can roll a replacement back to the last synchronized
state/root pair, and cleanup cannot remove the inactive slot until validation
and ordered resynchronization have completed.

Every reviewed declaration below carries a deductive TLAPS proof body.  The
safety closure uses an explicit initial-state decomposition, action-by-action
preservation, and the invariant rule.  The two responsive local corridors use
only their reviewed weak-fair actions; in particular, the durable-output proof
counts both worker-clear transitions separately.  These local results are not
a Rust-to-TLA refinement or an unbounded distributed-progress claim.
***************************************************************************)

THEOREM TypedRolloverInitEstablishesSafetyObligation ==
  Init => TypedRolloverSafetyInvariant
PROOF
  <1>1. Init => TypedRolloverTypeInvariant
    BY IsaT(300)
       DEF Init, TypedRolloverTypeInvariant, StartupModes,
           LifecycleValidationFaults, Rosters, CompactionCauses,
           OwnerNonces, HandoffCandidateKinds, Contexts, Artifacts,
           Parents, Successors, ReceiptStages, RequesterEpochPhases,
           LifecycleEntryStates, LifecycleRootV3Set,
           LifecycleRootShapes, LifecycleSnapshotV3Set,
           LifecycleStateSlotsV3Set, LifecycleCommitPhases,
           PendingRolloverAuthorities, ChangedRosterAuthorities,
           TransitionAuthorities, FailureReasons,
           InitialLifecycleSnapshotV3, LiveLifecycleSnapshotV3,
           InitialLifecycleStateSlotsV3, LifecycleSnapshotV3,
           LifecycleRootV3, BootstrapLifecycleRootV3,
           LifecycleSnapshotDigest, LifecycleStateSlot,
           NoLifecycleSnapshot
  <1>2. Init => CompactionGeometryInvariant
    BY IsaT(120)
       DEF Init, CompactionGeometryInvariant,
           CompactionCauseMatchesGeometry
  <1>3. Init => ExactServiceTransportOwnerPairInvariant
    BY IsaT(120)
       DEF Init, ExactServiceTransportOwnerPairInvariant,
           ExactServiceTransportOwnerPair
  <1>4. Init => UnconsumedPredecessorTransportOwnershipInvariant
    BY IsaT(120)
       DEF Init, UnconsumedPredecessorTransportOwnershipInvariant
  <1>5. Init => ReceiptLifecycleInvariant
    BY IsaT(120)
       DEF Init, ReceiptLifecycleInvariant,
           ExactPredecessorReceipt, ExactServiceTransportOwnerPair,
           ExactSuccessorConstruction
  <1>6. Init => FinalSealRejectsLateEnqueueInvariant
    BY IsaT(120)
       DEF Init, FinalSealRejectsLateEnqueueInvariant,
           FinalExactOutputSeal
  <1>7. Init => MismatchRejectionInvariant
    BY IsaT(120) DEF Init, MismatchRejectionInvariant
  <1>8. Init => FailureLatchInvariant
    BY IsaT(120) DEF Init, FailureLatchInvariant
  <1>9. Init => RootAnchoredLifecycleV3Invariant
    BY IsaT(600)
       DEF Init, RootAnchoredLifecycleV3Invariant,
           LifecycleRootV3Set, LifecycleRootShapes,
           LifecycleStateSlotsV3Set, LifecycleSnapshotV3Set,
           LifecycleRootShapeIsValid,
           RootSelectedLifecyclePairMatches,
           RootSelectedLifecyclePairIsPresent,
           SyncedRootSelectedLifecyclePairMatches,
           LifecycleSnapshotSemanticallyValid,
           DurableSnapshot, SyncedLifecycleSnapshot,
           SelectedLifecycleSnapshotV3,
           SyncedSelectedLifecycleSnapshotV3,
           SelectedLifecycleStateSlot,
           SyncedSelectedLifecycleStateSlot,
           LifecycleStateDirectoryIsSynced,
           LifecycleRootDirectoryIsSynced,
           InitialLifecycleSnapshotV3, LiveLifecycleSnapshotV3,
           InitialLifecycleStateSlotsV3, LifecycleSnapshotV3,
           LifecycleRootV3, BootstrapLifecycleRootV3,
           LifecycleSnapshotDigest, LifecycleStateSlot,
           NoLifecycleSnapshot
  <1>10. Init => SemanticValidationBeforeCleanupInvariant
    BY IsaT(300)
       DEF Init, SemanticValidationBeforeCleanupInvariant,
           LifecycleStateDirectoryIsSynced,
           LifecycleRootDirectoryIsSynced,
           RootSelectedLifecyclePairMatches,
           RootSelectedLifecyclePairIsPresent,
           LifecycleSnapshotSemanticallyValid, DurableSnapshot,
           SelectedLifecycleSnapshotV3, SelectedLifecycleStateSlot,
           InitialLifecycleSnapshotV3, LiveLifecycleSnapshotV3,
           InitialLifecycleStateSlotsV3, LifecycleSnapshotV3,
           LifecycleRootV3, BootstrapLifecycleRootV3,
           LifecycleSnapshotDigest, LifecycleStateSlot,
           NoLifecycleSnapshot
  <1>11. Init => ValidatedCleanupRemovesInactiveSlotInvariant
    BY IsaT(120)
       DEF Init, ValidatedCleanupRemovesInactiveSlotInvariant
  <1>12. Init => InvalidLifecycleStartupFailsClosedInvariant
    BY IsaT(180)
       DEF Init, InvalidLifecycleStartupFailsClosedInvariant
  <1>13. Init => ProcessLocalAuthorityAfterCrashInvariant
    BY IsaT(180)
       DEF Init, ProcessLocalAuthorityAfterCrashInvariant
  <1>14. Init => LifecycleCommitPhaseInvariant
    BY IsaT(600)
       DEF Init, LifecycleCommitPhaseInvariant,
           LifecycleTablesMatchDurableSnapshotV3,
           LifecycleMemoryMatchesDurableSnapshotV3,
           PersistentLifecycleMemoryMatchesSnapshot,
           RequesterEpochPersistenceAheadOfMemory,
           DurableCandidateStateSlotAheadOfRoot,
           RootSelectedSuccessorAheadOfMemory,
           RootCommittedSuccessorAheadOfMemory,
           RootSelectedLifecyclePairMatches,
           RootSelectedLifecyclePairIsPresent,
           LifecycleSnapshotSemanticallyValid, DurableSnapshot,
           SelectedLifecycleSnapshotV3, SelectedLifecycleStateSlot,
           LifecycleStateDirectoryIsSynced,
           LifecycleRootDirectoryIsSynced,
           InitialLifecycleSnapshotV3, LiveLifecycleSnapshotV3,
           InitialLifecycleStateSlotsV3, LifecycleSnapshotV3,
           LifecycleRootV3, BootstrapLifecycleRootV3,
           LifecycleSnapshotDigest, LifecycleStateSlot,
           NoLifecycleSnapshot
  <1>15. Init => AuthorityGatedGenerationAdvanceInvariant
    BY IsaT(300)
       DEF Init, AuthorityGatedGenerationAdvanceInvariant,
           ChangedRosterReplacementNeeded,
           CompactionCauseMatchesGeometry,
           AllOldLifecycleTerminal,
           ExactRetainedMergeSidecars, ExactPredecessorReceipt,
           ExactServiceTransportOwnerPair, ExactSuccessorConstruction,
           DurableSnapshot, SelectedLifecycleSnapshotV3,
           SelectedLifecycleStateSlot,
           InitialLifecycleSnapshotV3, LiveLifecycleSnapshotV3,
           InitialLifecycleStateSlotsV3, LifecycleSnapshotV3,
           LifecycleRootV3, LifecycleSnapshotDigest,
           LifecycleStateSlot, NoLifecycleSnapshot
  <1>16. Init => SameRosterTransportPreservationInvariant
    BY IsaT(300)
       DEF Init, SameRosterTransportPreservationInvariant,
           DurableSnapshot, SelectedLifecycleSnapshotV3,
           SelectedLifecycleStateSlot,
           InitialLifecycleSnapshotV3, LiveLifecycleSnapshotV3,
           InitialLifecycleStateSlotsV3, LifecycleSnapshotV3,
           LifecycleRootV3, LifecycleSnapshotDigest,
           LifecycleStateSlot, NoLifecycleSnapshot
  <1>17. Init => RequesterEpochInvariant
    BY IsaT(180)
       DEF Init, RequesterEpochInvariant, DurableSnapshot,
           SelectedLifecycleSnapshotV3, SelectedLifecycleStateSlot,
           InitialLifecycleSnapshotV3, LiveLifecycleSnapshotV3,
           InitialLifecycleStateSlotsV3, LifecycleSnapshotV3,
           LifecycleRootV3, LifecycleSnapshotDigest,
           LifecycleStateSlot, NoLifecycleSnapshot
  <1>18. Init => CrashRecoveryInvariant
    BY IsaT(120) DEF Init, CrashRecoveryInvariant
  <1>19. Init => NoForgedAuthenticatedClosePrefixInvariant
    BY IsaT(240)
       DEF Init, NoForgedAuthenticatedClosePrefixInvariant,
           DurableSnapshot, SelectedLifecycleSnapshotV3,
           SelectedLifecycleStateSlot,
           InitialLifecycleSnapshotV3, LiveLifecycleSnapshotV3,
           InitialLifecycleStateSlotsV3, LifecycleSnapshotV3,
           LifecycleRootV3, LifecycleSnapshotDigest,
           LifecycleStateSlot, NoLifecycleSnapshot
  <1>20. Init => LateOldCallbackIsolationInvariant
    BY IsaT(120) DEF Init, LateOldCallbackIsolationInvariant
  <1> QED BY <1>1, <1>2, <1>3, <1>4, <1>5,
                 <1>6, <1>7, <1>8, <1>9, <1>10,
                 <1>11, <1>12, <1>13, <1>14, <1>15,
                 <1>16, <1>17, <1>18, <1>19, <1>20
       DEF TypedRolloverSafetyInvariant

THEOREM BootstrapRootHasExactGenerationZeroShapeObligation ==
  /\ Init
  /\ state.startupMode = "BootstrapStart"
  =>
    /\ state.durableLifecycleRootV3 = BootstrapLifecycleRootV3
    /\ state.syncedLifecycleRootV3 = BootstrapLifecycleRootV3
    /\ \A slot \in StateSlots:
         state.durableLifecycleStateSlotsV3[slot] =
           NoLifecycleSnapshot
    /\ state.durableLifecycleStateSlotsV3 =
         state.syncedLifecycleStateSlotsV3
    /\ state.lifecycleCommitPhase = "Bootstrap"
PROOF
  BY Isa DEF Init, BootstrapLifecycleRootV3

THEOREM FreshBootstrapUsesTargetGeometryEpochZeroObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ PublishInitialLifecycleStateSlotV3
  =>
    /\ state'.candidateLifecycleSnapshotV3 =
         InitialLifecycleSnapshotV3(state.targetRoster)
    /\ state'.candidateLifecycleSnapshotV3.roster =
         state.targetRoster
    /\ state'.candidateLifecycleSnapshotV3.serviceGeneration =
         InitialServiceGeneration
    /\ state'.candidateLifecycleSnapshotV3.nextStreamEpoch = 0
    /\ state'.candidateLifecycleSnapshotV3.requesterStreamEpoch = 0
    /\ state'.candidateLifecycleSnapshotV3.serverStreams = "Empty"
    /\ state'.candidateLifecycleSnapshotV3.requestGates = "Empty"
    /\ state'.candidateLifecycleSnapshotV3.serverClosePrefix = 0
PROOF
  BY Isa DEF PublishInitialLifecycleStateSlotV3,
                 InitialLifecycleSnapshotV3, LifecycleSnapshotV3

THEOREM BootstrapStateReplacementRequiresDirectorySyncObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ ReplaceInitialLifecycleRootV3
  =>
    /\ LifecycleStateDirectoryIsSynced(state)
    /\ state.candidateSemanticallyValidated
    /\ state'.durableLifecycleRootV3 =
         LifecycleRootV3(state.candidateLifecycleSnapshotV3)
    /\ state'.syncedLifecycleRootV3 =
         state.syncedLifecycleRootV3
PROOF
  BY Isa DEF ReplaceInitialLifecycleRootV3

THEOREM BootstrapCrashRecoveryObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ (CrashAfterBootstrapStateReplacement
       \/ CrashAfterBootstrapStatePublication
       \/ CrashAfterBootstrapRootReplacement)
  =>
    /\ state'.restartRequired
    /\ ~state'.durableJournalValidated
    /\ state'.bootstrapCrashObserved
    /\ ~state'.candidatePresent
    /\ state'.serviceOwnerNonce = NoIdentity
    /\ state'.transportOwnerNonce = NoIdentity
    /\ state'.receiptStage \in {"Absent", "Lost"}
PROOF
  BY Isa DEF CrashAfterBootstrapStateReplacement,
                 CrashAfterBootstrapStatePublication,
                 CrashAfterBootstrapRootReplacement,
                 CrashClearedProcessLocalRolloverState

THEOREM BootstrapFirstCommitSelectsExactInitialPairObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ CommitInitialLifecycleRootV3
  =>
    /\ state'.durableLifecycleRootV3 =
         LifecycleRootV3(
           InitialLifecycleSnapshotV3(state.targetRoster))
    /\ state'.syncedLifecycleRootV3 =
         state'.durableLifecycleRootV3
    /\ SelectedLifecycleStateSlot(state') =
         LifecycleStateSlot(InitialRootGeneration)
    /\ SelectedLifecycleSnapshotV3(state') =
         InitialLifecycleSnapshotV3(state.targetRoster)
    /\ RootSelectedLifecyclePairMatches(state')
    /\ LifecycleMemory(state') = LifecycleMemory(state)
PROOF
  BY Isa DEF CommitInitialLifecycleRootV3,
                 TypedRolloverSafetyInvariant,
                 LifecycleCommitPhaseInvariant,
                 RootAnchoredLifecycleV3Invariant,
                 RootSelectedLifecyclePairMatches,
                 RootSelectedLifecyclePairIsPresent,
                 DurableSnapshot, SelectedLifecycleStateSlot,
                 SelectedLifecycleSnapshotV3, LifecycleStateSlot,
                 LifecycleRootV3, LifecycleSnapshotDigest,
                 InitialLifecycleSnapshotV3, LifecycleSnapshotV3,
                 LifecycleMemory

THEOREM ExactOwnerPairRequiredForRetainedHandoffObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ RetainExactHandoffReceipt
  =>
    /\ state.serviceOwnerNonce # NoIdentity
    /\ state.serviceOwnerNonce = state.transportOwnerNonce
    /\ state'.receiptOwnerNonce = state.serviceOwnerNonce
    /\ state'.receiptStage = "Retained"
PROOF
  BY Isa DEF RetainExactHandoffReceipt, ExactPredecessorReceipt,
                 ExactServiceTransportOwnerPair

(***************************************************************************
Single-action inductive closure.

The common USE block unfolds the reviewed invariant once.  Each leaf then
opens exactly one action, so no backend receives an opaque `Next` disjunction
or an unreviewed all-actions shortcut.
***************************************************************************)
THEOREM TypedRolloverNextPreservesSafetyObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ Next
  =>
    TypedRolloverSafetyInvariant'
PROOF
  <1> USE DEF TypedRolloverSafetyInvariant,
              TypedRolloverTypeInvariant,
              CompactionGeometryInvariant,
              CompactionCauseMatchesGeometry,
              ExactServiceTransportOwnerPairInvariant,
              ExactServiceTransportOwnerPair,
              UnconsumedPredecessorTransportOwnershipInvariant,
              PredecessorTransportOwnershipOpen,
              ReceiptLifecycleInvariant, ExactPredecessorReceipt,
              ExactSuccessorConstruction, ExactRetainedMergeSidecars,
              FinalSealRejectsLateEnqueueInvariant,
              FinalExactOutputSeal, MismatchRejectionInvariant,
              FailureLatchInvariant, RootAnchoredLifecycleV3Invariant,
              SemanticValidationBeforeCleanupInvariant,
              ValidatedCleanupRemovesInactiveSlotInvariant,
              InvalidLifecycleStartupFailsClosedInvariant,
              ProcessLocalAuthorityAfterCrashInvariant,
              LifecycleCommitPhaseInvariant,
              AuthorityGatedGenerationAdvanceInvariant,
              SameRosterTransportPreservationInvariant,
              RequesterEpochInvariant, CrashRecoveryInvariant,
              NoForgedAuthenticatedClosePrefixInvariant,
              LateOldCallbackIsolationInvariant,
              ChangedRosterReplacementNeeded, SameRosterTableFull,
              AllOldLifecycleTerminal, ChangedRosterAuthorityAvailable,
              PersistentLifecycleMemoryMatchesSnapshot,
              LifecycleMemoryMatchesDurableSnapshotV3,
              LifecycleTablesMatchDurableSnapshotV3,
              DurableCandidateStateSlotAheadOfRoot,
              ValidatedCandidateSuccessorStateSlotAheadOfRoot,
              RootCommittedSuccessorAheadOfMemory,
              RootSelectedSuccessorAheadOfMemory,
              RequesterEpochPersistenceAheadOfMemory,
              LifecycleJournalReady, LifecycleRootShapeIsValid,
              RootSelectedLifecyclePairIsPresent,
              RootSelectedLifecyclePairMatches,
              SyncedRootSelectedLifecyclePairMatches,
              LifecycleSnapshotSemanticallyValid,
              LifecycleStateDirectoryIsSynced,
              LifecycleRootDirectoryIsSynced,
              InactiveLifecycleArtifactPresent,
              DurableSnapshot, SyncedLifecycleSnapshot,
              SelectedLifecycleStateSlot,
              SelectedLifecycleSnapshotV3,
              SyncedSelectedLifecycleStateSlot,
              SyncedSelectedLifecycleSnapshotV3,
              InactiveLifecycleStateSlot, LifecycleStateSlot,
              LifecycleRootV3, LifecycleSnapshotV3,
              LifecycleSnapshotDigest, BootstrapLifecycleRootV3,
              InitialLifecycleSnapshotV3, LiveLifecycleSnapshotV3,
              InitialLifecycleStateSlotsV3, NoLifecycleSnapshot,
              LifecycleRootV3Set, LifecycleSnapshotV3Set,
              LifecycleStateSlotsV3Set, LifecycleRootShapes,
              StartupModes, LifecycleValidationFaults, Rosters,
              CompactionCauses, OwnerNonces, HandoffCandidateKinds,
              Contexts, Artifacts, Parents, Successors, ReceiptStages,
              RequesterEpochPhases, LifecycleEntryStates,
              LifecycleCommitPhases, PendingRolloverAuthorities,
              ChangedRosterAuthorities, TransitionAuthorities,
              FailureReasons
  <1>1. CASE CreateServiceTransportOwnerPair
    BY <1>1, IsaT(600) DEF CreateServiceTransportOwnerPair
  <1>2. CASE ValidateFinality
    BY <1>2, IsaT(600) DEF ValidateFinality
  <1>3. CASE CloseWorkerIngress
    BY <1>3, IsaT(600) DEF CloseWorkerIngress
  <1>4. CASE ClearOneWorkerExactOutput
    BY <1>4, IsaT(600) DEF ClearOneWorkerExactOutput
  <1>5. CASE BuildImmediateSuccessor
    BY <1>5, IsaT(600) DEF BuildImmediateSuccessor
  <1>6. CASE SealAppliedHeightOutputHandoff
    BY <1>6, IsaT(600) DEF SealAppliedHeightOutputHandoff
  <1>7. CASE RejectLateExactOutputEnqueue
    BY <1>7, IsaT(600) DEF RejectLateExactOutputEnqueue
  <1>8. CASE PresentForeignOwnerReceiptCandidate
    BY <1>8, IsaT(600) DEF PresentForeignOwnerReceiptCandidate
  <1>9. CASE PresentPredecessorContextMismatchCandidate
    BY <1>9, IsaT(600)
       DEF PresentPredecessorContextMismatchCandidate
  <1>10. CASE PresentPredecessorArtifactMismatchCandidate
    BY <1>10, IsaT(600)
       DEF PresentPredecessorArtifactMismatchCandidate
  <1>11. CASE PresentWrongImmediateSuccessorCandidate
    BY <1>11, IsaT(600)
       DEF PresentWrongImmediateSuccessorCandidate
  <1>12. CASE RejectForeignOwnerReceipt
    BY <1>12, IsaT(600) DEF RejectForeignOwnerReceipt
  <1>13. CASE RejectPredecessorContextMismatch
    BY <1>13, IsaT(600) DEF RejectPredecessorContextMismatch
  <1>14. CASE RejectPredecessorArtifactMismatch
    BY <1>14, IsaT(600) DEF RejectPredecessorArtifactMismatch
  <1>15. CASE RejectWrongImmediateSuccessor
    BY <1>15, IsaT(600) DEF RejectWrongImmediateSuccessor
  <1>16. CASE RetainExactHandoffReceipt
    BY <1>16, IsaT(600) DEF RetainExactHandoffReceipt
  <1>17. CASE PublishInitialLifecycleStateSlotV3
    BY <1>17, IsaT(900) DEF PublishInitialLifecycleStateSlotV3
  <1>18. CASE SyncInitialLifecycleStateDirectoryV3
    BY <1>18, IsaT(900) DEF SyncInitialLifecycleStateDirectoryV3
  <1>19. CASE CrashAfterBootstrapStateReplacement
    BY <1>19, IsaT(900)
       DEF CrashAfterBootstrapStateReplacement,
           CrashClearedProcessLocalRolloverState
  <1>20. CASE CrashAfterBootstrapStatePublication
    BY <1>20, IsaT(900)
       DEF CrashAfterBootstrapStatePublication,
           CrashClearedProcessLocalRolloverState
  <1>21. CASE ValidateBootstrapLifecycleCandidateV3
    BY <1>21, IsaT(900) DEF ValidateBootstrapLifecycleCandidateV3
  <1>22. CASE ValidateBootstrapLifecycleWithoutCandidateV3
    BY <1>22, IsaT(900)
       DEF ValidateBootstrapLifecycleWithoutCandidateV3
  <1>23. CASE ReplaceInitialLifecycleRootV3
    BY <1>23, IsaT(900) DEF ReplaceInitialLifecycleRootV3
  <1>24. CASE CrashAfterBootstrapRootReplacement
    BY <1>24, IsaT(900)
       DEF CrashAfterBootstrapRootReplacement,
           CrashClearedProcessLocalRolloverState
  <1>25. CASE CommitInitialLifecycleRootV3
    BY <1>25, IsaT(900) DEF CommitInitialLifecycleRootV3
  <1>26. CASE ValidateRootSelectedLifecycleV3
    BY <1>26, IsaT(900) DEF ValidateRootSelectedLifecycleV3
  <1>27. CASE RejectLifecycleRootShapeMismatchV3
    BY <1>27, IsaT(600)
       DEF RejectLifecycleRootShapeMismatchV3,
           RejectInvalidLifecycleStartupV3
  <1>28. CASE RejectLifecycleSelectedStateMissingV3
    BY <1>28, IsaT(600)
       DEF RejectLifecycleSelectedStateMissingV3,
           RejectInvalidLifecycleStartupV3
  <1>29. CASE RejectLifecycleGenerationHashMismatchV3
    BY <1>29, IsaT(600)
       DEF RejectLifecycleGenerationHashMismatchV3,
           RejectInvalidLifecycleStartupV3
  <1>30. CASE RejectLifecycleSemanticValidationFailureV3
    BY <1>30, IsaT(600)
       DEF RejectLifecycleSemanticValidationFailureV3,
           RejectInvalidLifecycleStartupV3
  <1>31. CASE ResyncValidatedLifecycleStateDirectoryV3
    BY <1>31, IsaT(900)
       DEF ResyncValidatedLifecycleStateDirectoryV3
  <1>32. CASE ResyncValidatedLifecycleRootDirectoryV3
    BY <1>32, IsaT(900)
       DEF ResyncValidatedLifecycleRootDirectoryV3
  <1>33. CASE CrashDuringValidatedRestartBeforeRootResyncV3
    BY <1>33, IsaT(900)
       DEF CrashDuringValidatedRestartBeforeRootResyncV3,
           CrashClearedProcessLocalRolloverState
  <1>34. CASE CleanupValidatedLifecycleArtifactsV3
    BY <1>34, IsaT(900) DEF CleanupValidatedLifecycleArtifactsV3
  <1>35. CASE CompleteBootstrapRestartWithoutCandidateV3
    BY <1>35, IsaT(900)
       DEF CompleteBootstrapRestartWithoutCandidateV3
  <1>36. CASE PersistFreshRequesterEpoch
    BY <1>36, IsaT(900) DEF PersistFreshRequesterEpoch
  <1>37. CASE PublishFreshRequesterEpoch
    BY <1>37, IsaT(900) DEF PublishFreshRequesterEpoch
  <1>38. CASE CompleteRequesterEpoch
    BY <1>38, IsaT(600) DEF CompleteRequesterEpoch
  <1>39. CASE ReopenRequesterEpochAllocator
    BY <1>39, IsaT(600) DEF ReopenRequesterEpochAllocator
  <1>40. CASE CrashAfterRequesterEpochPersistence
    BY <1>40, IsaT(900)
       DEF CrashAfterRequesterEpochPersistence,
           CrashClearedProcessLocalRolloverState
  <1>41. CASE RestoreRequesterEpochCounterAfterCrash
    BY <1>41, IsaT(900)
       DEF RestoreRequesterEpochCounterAfterCrash
  <1>42. CASE RejectRequesterEpochOverflow
    BY <1>42, IsaT(600) DEF RejectRequesterEpochOverflow
  <1>43. CASE RejectActiveOrdinaryRollover
    BY <1>43, IsaT(600) DEF RejectActiveOrdinaryRollover
  <1>44. CASE RejectSameRosterFullTable
    BY <1>44, IsaT(600) DEF RejectSameRosterFullTable
  <1>45. CASE AuthenticateServerClosePrefix
    BY <1>45, IsaT(900) DEF AuthenticateServerClosePrefix
  <1>46. CASE TerminalizeRequestGate
    BY <1>46, IsaT(900) DEF TerminalizeRequestGate
  <1>47. CASE TerminalizeTransfer
    BY <1>47, IsaT(600) DEF TerminalizeTransfer
  <1>48. CASE TerminalizeFlush
    BY <1>48, IsaT(600) DEF TerminalizeFlush
  <1>49. CASE PublishSuccessorLifecycleStateSlotV3
    BY <1>49, IsaT(900)
       DEF PublishSuccessorLifecycleStateSlotV3,
           PublishSuccessorLifecycleStateSlotV3WithAuthority
  <1>50. CASE FailSuccessorLifecycleStateSlotV3Persistence
    BY <1>50, IsaT(900)
       DEF FailSuccessorLifecycleStateSlotV3Persistence,
           CrashClearedProcessLocalRolloverState
  <1>51. CASE CrashBeforeLifecycleStateSlotV3Publication
    BY <1>51, IsaT(900)
       DEF CrashBeforeLifecycleStateSlotV3Publication,
           CrashClearedProcessLocalRolloverState
  <1>52. CASE CrashAfterLifecycleStateReplacement
    BY <1>52, IsaT(900)
       DEF CrashAfterLifecycleStateReplacement,
           CrashClearedProcessLocalRolloverState
  <1>53. CASE SyncSuccessorLifecycleStateDirectoryV3
    BY <1>53, IsaT(900) DEF SyncSuccessorLifecycleStateDirectoryV3
  <1>54. CASE CrashAfterLifecycleStateSlotV3Publication
    BY <1>54, IsaT(900)
       DEF CrashAfterLifecycleStateSlotV3Publication,
           CrashClearedProcessLocalRolloverState
  <1>55. CASE FailSuccessorLifecycleRootV3Persistence
    BY <1>55, IsaT(900)
       DEF FailSuccessorLifecycleRootV3Persistence,
           CrashClearedProcessLocalRolloverState
  <1>56. CASE ReplaceSuccessorLifecycleRootV3
    BY <1>56, IsaT(900) DEF ReplaceSuccessorLifecycleRootV3
  <1>57. CASE CrashAfterLifecycleRootReplacement
    BY <1>57, IsaT(900)
       DEF CrashAfterLifecycleRootReplacement,
           CrashClearedProcessLocalRolloverState
  <1>58. CASE RecoverPredecessorLifecycleV3
    BY <1>58, IsaT(900) DEF RecoverPredecessorLifecycleV3
  <1>59. CASE CommitSuccessorLifecycleRootV3
    BY <1>59, IsaT(900) DEF CommitSuccessorLifecycleRootV3
  <1>60. CASE CleanupCommittedLifecyclePredecessorV3
    BY <1>60, IsaT(900)
       DEF CleanupCommittedLifecyclePredecessorV3
  <1>61. CASE CrashAfterLifecycleRootV3Commit
    BY <1>61, IsaT(900)
       DEF CrashAfterLifecycleRootV3Commit,
           CrashClearedProcessLocalRolloverState
  <1>62. CASE RestoreSuccessorLifecycleV3AfterCrash
    BY <1>62, IsaT(900) DEF RestoreSuccessorLifecycleV3AfterCrash
  <1>63. CASE PublishCommittedLifecycleV3ToMemory
    BY <1>63, IsaT(900) DEF PublishCommittedLifecycleV3ToMemory
  <1>64. CASE ActivateRestoredLifecycleV3Successor
    BY <1>64, IsaT(900) DEF ActivateRestoredLifecycleV3Successor
  <1>65. CASE ActivateSameRosterSuccessor
    BY <1>65, IsaT(900) DEF ActivateSameRosterSuccessor
  <1>66. CASE RejectServiceGenerationOverflow
    BY <1>66, IsaT(600) DEF RejectServiceGenerationOverflow
  <1>67. CASE FailLifecycleRootGenerationExhaustion
    BY <1>67, IsaT(900)
       DEF FailLifecycleRootGenerationExhaustion,
           CrashClearedProcessLocalRolloverState
  <1>68. CASE ObserveLateOldWriterCallback
    BY <1>68, IsaT(600) DEF ObserveLateOldWriterCallback
  <1> QED BY <1>1, <1>2, <1>3, <1>4, <1>5, <1>6, <1>7,
                 <1>8, <1>9, <1>10, <1>11, <1>12, <1>13,
                 <1>14, <1>15, <1>16, <1>17, <1>18, <1>19,
                 <1>20, <1>21, <1>22, <1>23, <1>24, <1>25,
                 <1>26, <1>27, <1>28, <1>29, <1>30, <1>31,
                 <1>32, <1>33, <1>34, <1>35, <1>36, <1>37,
                 <1>38, <1>39, <1>40, <1>41, <1>42, <1>43,
                 <1>44, <1>45, <1>46, <1>47, <1>48, <1>49,
                 <1>50, <1>51, <1>52, <1>53, <1>54, <1>55,
                 <1>56, <1>57, <1>58, <1>59, <1>60, <1>61,
                 <1>62, <1>63, <1>64, <1>65, <1>66, <1>67,
                 <1>68
       DEF Next

THEOREM EveryCrashDropsProcessLocalAuthorityObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ Next
  /\ state.durableJournalValidated
  /\ ~state'.durableJournalValidated
  /\ state'.restartRequired
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
PROOF
  BY TypedRolloverNextPreservesSafetyObligation, IsaT(120)
     DEF TypedRolloverSafetyInvariant,
         ProcessLocalAuthorityAfterCrashInvariant

THEOREM OnlyValidatedRestartMayFenceAfterCrashObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ Next
  /\ ~state.restartFenceAuthorized
  /\ state'.restartFenceAuthorized
  =>
    /\ state.durableJournalValidated
    /\ state.cleanupPerformed
    /\ state.restartStateDirectoryResynced
    /\ state.restartRootDirectoryResynced
    /\ state'.validatedRestartObserved
    /\ state'.targetRoster # state'.currentRoster
PROOF
  <1>1. CASE RecoverPredecessorLifecycleV3
    BY <1>1, IsaT(180) DEF RecoverPredecessorLifecycleV3
  <1>2. CASE RestoreRequesterEpochCounterAfterCrash
    BY <1>2, IsaT(180) DEF RestoreRequesterEpochCounterAfterCrash
  <1>3. CASE ~(RecoverPredecessorLifecycleV3
                 \/ RestoreRequesterEpochCounterAfterCrash)
    BY <1>3, IsaT(900)
       DEF TypedRolloverSafetyInvariant,
           AuthorityGatedGenerationAdvanceInvariant, Next,
           CreateServiceTransportOwnerPair, ValidateFinality,
           CloseWorkerIngress, ClearOneWorkerExactOutput,
           BuildImmediateSuccessor, SealAppliedHeightOutputHandoff,
           RejectLateExactOutputEnqueue,
           PresentForeignOwnerReceiptCandidate,
           PresentPredecessorContextMismatchCandidate,
           PresentPredecessorArtifactMismatchCandidate,
           PresentWrongImmediateSuccessorCandidate,
           RejectForeignOwnerReceipt,
           RejectPredecessorContextMismatch,
           RejectPredecessorArtifactMismatch,
           RejectWrongImmediateSuccessor, RetainExactHandoffReceipt,
           PublishInitialLifecycleStateSlotV3,
           SyncInitialLifecycleStateDirectoryV3,
           CrashAfterBootstrapStateReplacement,
           CrashAfterBootstrapStatePublication,
           ValidateBootstrapLifecycleCandidateV3,
           ValidateBootstrapLifecycleWithoutCandidateV3,
           ReplaceInitialLifecycleRootV3,
           CrashAfterBootstrapRootReplacement,
           CommitInitialLifecycleRootV3,
           ValidateRootSelectedLifecycleV3,
           RejectLifecycleRootShapeMismatchV3,
           RejectLifecycleSelectedStateMissingV3,
           RejectLifecycleGenerationHashMismatchV3,
           RejectLifecycleSemanticValidationFailureV3,
           RejectInvalidLifecycleStartupV3,
           ResyncValidatedLifecycleStateDirectoryV3,
           ResyncValidatedLifecycleRootDirectoryV3,
           CrashDuringValidatedRestartBeforeRootResyncV3,
           CleanupValidatedLifecycleArtifactsV3,
           CompleteBootstrapRestartWithoutCandidateV3,
           PersistFreshRequesterEpoch, PublishFreshRequesterEpoch,
           CompleteRequesterEpoch, ReopenRequesterEpochAllocator,
           CrashAfterRequesterEpochPersistence,
           RejectRequesterEpochOverflow,
           RejectActiveOrdinaryRollover, RejectSameRosterFullTable,
           AuthenticateServerClosePrefix, TerminalizeRequestGate,
           TerminalizeTransfer, TerminalizeFlush,
           PublishSuccessorLifecycleStateSlotV3,
           PublishSuccessorLifecycleStateSlotV3WithAuthority,
           FailSuccessorLifecycleStateSlotV3Persistence,
           CrashBeforeLifecycleStateSlotV3Publication,
           CrashAfterLifecycleStateReplacement,
           SyncSuccessorLifecycleStateDirectoryV3,
           CrashAfterLifecycleStateSlotV3Publication,
           FailSuccessorLifecycleRootV3Persistence,
           ReplaceSuccessorLifecycleRootV3,
           CrashAfterLifecycleRootReplacement,
           CommitSuccessorLifecycleRootV3,
           CleanupCommittedLifecyclePredecessorV3,
           CrashAfterLifecycleRootV3Commit,
           RestoreSuccessorLifecycleV3AfterCrash,
           PublishCommittedLifecycleV3ToMemory,
           ActivateRestoredLifecycleV3Successor,
           ActivateSameRosterSuccessor,
           RejectServiceGenerationOverflow,
           FailLifecycleRootGenerationExhaustion,
           ObserveLateOldWriterCallback,
           CrashClearedProcessLocalRolloverState
  <1> QED BY <1>1, <1>2, <1>3

THEOREM StateReplacementRequiresDirectorySyncBeforeRootReplacementObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ ReplaceSuccessorLifecycleRootV3
  =>
    /\ state.lifecycleCommitPhase = "StateSlotPublished"
    /\ LifecycleStateDirectoryIsSynced(state)
    /\ state.candidateSemanticallyValidated
    /\ state'.durableLifecycleRootV3 =
         LifecycleRootV3(state.candidateLifecycleSnapshotV3)
    /\ state'.syncedLifecycleRootV3 =
         state.syncedLifecycleRootV3
    /\ LifecycleMemory(state') = LifecycleMemory(state)
PROOF
  BY Isa DEF ReplaceSuccessorLifecycleRootV3, LifecycleMemory

THEOREM RootReplacementRequiresStoreSyncBeforeMemoryPublicationObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ PublishCommittedLifecycleV3ToMemory
  =>
    /\ state.lifecycleCommitPhase = "RootCommitted"
    /\ LifecycleStateDirectoryIsSynced(state)
    /\ LifecycleRootDirectoryIsSynced(state)
    /\ RootSelectedLifecyclePairMatches(state)
    /\ state'.serviceGeneration =
         DurableSnapshot(state).serviceGeneration
    /\ state'.lifecycleCommitPhase = "Published"
PROOF
  BY Isa DEF PublishCommittedLifecycleV3ToMemory,
                 RootCommittedSuccessorAheadOfMemory

THEOREM RootSelectedPairBindsGenerationAndDigestObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ state.durableLifecycleRootV3.shape = "Committed"
  =>
    /\ RootSelectedLifecyclePairIsPresent(state)
    /\ SelectedLifecycleSnapshotV3(state).rootGeneration =
         state.durableLifecycleRootV3.rootGeneration
    /\ state.durableLifecycleRootV3.snapshotDigest =
         LifecycleSnapshotDigest(
           SelectedLifecycleSnapshotV3(state))
    /\ LifecycleSnapshotSemanticallyValid(
         SelectedLifecycleSnapshotV3(state))
PROOF
  BY SMT DEF TypedRolloverSafetyInvariant,
                 RootAnchoredLifecycleV3Invariant,
                 RootSelectedLifecyclePairMatches

THEOREM MissingRootSelectedStateCannotValidateOrCleanupObligation ==
  /\ ~RootSelectedLifecyclePairIsPresent(state)
  /\ (ValidateRootSelectedLifecycleV3
       \/ CleanupValidatedLifecycleArtifactsV3)
  =>
    FALSE
PROOF
  BY SMT DEF ValidateRootSelectedLifecycleV3,
                 CleanupValidatedLifecycleArtifactsV3,
                 RootSelectedLifecyclePairMatches,
                 RootSelectedLifecyclePairIsPresent

THEOREM ValidationFailurePreservesArtifactsObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ ~state.validationFailureObserved
  /\ state'.validationFailureObserved
  /\ Next
  =>
    /\ DurableLifecycle(state') = DurableLifecycle(state)
    /\ CandidateLifecycle(state') = CandidateLifecycle(state)
    /\ state'.crashArtifactsPresent
    /\ ~state'.cleanupPerformed
    /\ ~state'.durableJournalValidated
    /\ ~state'.successorActive
PROOF
  BY Isa DEF TypedRolloverSafetyInvariant, FailureLatchInvariant, Next,
                 CreateServiceTransportOwnerPair, ValidateFinality,
                 CloseWorkerIngress, ClearOneWorkerExactOutput,
                 BuildImmediateSuccessor, SealAppliedHeightOutputHandoff,
                 RejectLateExactOutputEnqueue,
                 PresentForeignOwnerReceiptCandidate,
                 PresentPredecessorContextMismatchCandidate,
                 PresentPredecessorArtifactMismatchCandidate,
                 PresentWrongImmediateSuccessorCandidate,
                 RejectForeignOwnerReceipt,
                 RejectPredecessorContextMismatch,
                 RejectPredecessorArtifactMismatch,
                 RejectWrongImmediateSuccessor, RetainExactHandoffReceipt,
                 PublishInitialLifecycleStateSlotV3,
                 SyncInitialLifecycleStateDirectoryV3,
                 CrashAfterBootstrapStateReplacement,
                 CrashAfterBootstrapStatePublication,
                 ValidateBootstrapLifecycleCandidateV3,
                 ValidateBootstrapLifecycleWithoutCandidateV3,
                 ReplaceInitialLifecycleRootV3,
                 CrashAfterBootstrapRootReplacement,
                 CommitInitialLifecycleRootV3,
                 ValidateRootSelectedLifecycleV3,
                 RejectLifecycleRootShapeMismatchV3,
                 RejectLifecycleSelectedStateMissingV3,
                 RejectLifecycleGenerationHashMismatchV3,
                 RejectLifecycleSemanticValidationFailureV3,
                 RejectInvalidLifecycleStartupV3,
                 ResyncValidatedLifecycleStateDirectoryV3,
                 ResyncValidatedLifecycleRootDirectoryV3,
                 CrashDuringValidatedRestartBeforeRootResyncV3,
                 CleanupValidatedLifecycleArtifactsV3,
                 CompleteBootstrapRestartWithoutCandidateV3,
                 PersistFreshRequesterEpoch, PublishFreshRequesterEpoch,
                 CompleteRequesterEpoch, ReopenRequesterEpochAllocator,
                 CrashAfterRequesterEpochPersistence,
                 RestoreRequesterEpochCounterAfterCrash,
                 RejectRequesterEpochOverflow,
                 RejectActiveOrdinaryRollover, RejectSameRosterFullTable,
                 AuthenticateServerClosePrefix, TerminalizeRequestGate,
                 TerminalizeTransfer, TerminalizeFlush,
                 PublishSuccessorLifecycleStateSlotV3,
                 PublishSuccessorLifecycleStateSlotV3WithAuthority,
                 FailSuccessorLifecycleStateSlotV3Persistence,
                 CrashBeforeLifecycleStateSlotV3Publication,
                 CrashAfterLifecycleStateReplacement,
                 SyncSuccessorLifecycleStateDirectoryV3,
                 CrashAfterLifecycleStateSlotV3Publication,
                 FailSuccessorLifecycleRootV3Persistence,
                 ReplaceSuccessorLifecycleRootV3,
                 CrashAfterLifecycleRootReplacement,
                 RecoverPredecessorLifecycleV3,
                 CommitSuccessorLifecycleRootV3,
                 CleanupCommittedLifecyclePredecessorV3,
                 CrashAfterLifecycleRootV3Commit,
                 RestoreSuccessorLifecycleV3AfterCrash,
                 PublishCommittedLifecycleV3ToMemory,
                 ActivateRestoredLifecycleV3Successor,
                 ActivateSameRosterSuccessor,
                 RejectServiceGenerationOverflow,
                 FailLifecycleRootGenerationExhaustion,
                 ObserveLateOldWriterCallback,
                 CrashClearedProcessLocalRolloverState,
                 DurableLifecycle, CandidateLifecycle

THEOREM SemanticValidationPrecedesArtifactCleanupObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ CleanupValidatedLifecycleArtifactsV3
  =>
    /\ state.durableJournalValidated
    /\ RootSelectedLifecyclePairMatches(state)
    /\ LifecycleSnapshotSemanticallyValid(DurableSnapshot(state))
    /\ state.restartStateDirectoryResynced
    /\ state.restartRootDirectoryResynced
    /\ state'.cleanupPerformed
    /\ ~state'.crashArtifactsPresent
PROOF
  BY Isa DEF CleanupValidatedLifecycleArtifactsV3

THEOREM ValidatedCleanupRemovesInactiveSlotObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ CleanupValidatedLifecycleArtifactsV3
  =>
    /\ state'.durableLifecycleStateSlotsV3[
         InactiveLifecycleStateSlot(state')] =
         NoLifecycleSnapshot
    /\ state'.syncedLifecycleStateSlotsV3[
         InactiveLifecycleStateSlot(state')] =
         NoLifecycleSnapshot
    /\ LifecycleStateDirectoryIsSynced(state')
    /\ LifecycleRootDirectoryIsSynced(state')
PROOF
  BY Isa DEF CleanupValidatedLifecycleArtifactsV3,
                 InactiveLifecycleStateSlot,
                 SelectedLifecycleStateSlot,
                 LifecycleStateDirectoryIsSynced,
                 LifecycleRootDirectoryIsSynced

THEOREM SecondCrashBeforeRootResyncPreservesPredecessorObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ CrashDuringValidatedRestartBeforeRootResyncV3
  =>
    /\ state'.durableLifecycleRootV3 =
         state.syncedLifecycleRootV3
    /\ state'.durableLifecycleStateSlotsV3 =
         state.syncedLifecycleStateSlotsV3
    /\ state'.durableLifecycleRootV3 =
         state'.syncedLifecycleRootV3
    /\ state'.durableLifecycleStateSlotsV3 =
         state'.syncedLifecycleStateSlotsV3
    /\ state'.restartRequired
    /\ ~state'.durableJournalValidated
    /\ state'.secondCrashObserved
    /\ ~state'.cleanupPerformed
PROOF
  BY Isa DEF CrashDuringValidatedRestartBeforeRootResyncV3,
                 CrashClearedProcessLocalRolloverState

THEOREM RootGenerationAdvancesExactlyOnceAndAlternatesSlotObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ Next
  /\ state'.syncedLifecycleRootV3.rootGeneration >
       state.syncedLifecycleRootV3.rootGeneration
  =>
    /\ state'.syncedLifecycleRootV3.rootGeneration =
         state.syncedLifecycleRootV3.rootGeneration + 1
    /\ SyncedSelectedLifecycleStateSlot(state') =
         LifecycleStateSlot(
           state'.syncedLifecycleRootV3.rootGeneration)
    /\ SyncedSelectedLifecycleStateSlot(state') #
         SyncedSelectedLifecycleStateSlot(state)
    /\ state'.syncedLifecycleRootV3.snapshotDigest =
         LifecycleSnapshotDigest(
           SyncedSelectedLifecycleSnapshotV3(state'))
PROOF
  <1>1. CASE CommitInitialLifecycleRootV3
    BY <1>1, IsaT(600)
       DEF TypedRolloverSafetyInvariant,
           RootAnchoredLifecycleV3Invariant,
           SyncedRootSelectedLifecyclePairMatches,
           CommitInitialLifecycleRootV3,
           RootSelectedLifecyclePairMatches,
           RootSelectedLifecyclePairIsPresent,
           SyncedSelectedLifecycleStateSlot,
           SyncedSelectedLifecycleSnapshotV3,
           SelectedLifecycleStateSlot,
           SelectedLifecycleSnapshotV3,
           LifecycleStateSlot, LifecycleRootV3,
           LifecycleSnapshotDigest, BootstrapLifecycleRootV3
  <1>2. CASE ResyncValidatedLifecycleRootDirectoryV3
    BY <1>2, IsaT(600)
       DEF TypedRolloverSafetyInvariant,
           RootAnchoredLifecycleV3Invariant,
           SyncedRootSelectedLifecyclePairMatches,
           RootSelectedLifecyclePairMatches,
           RootSelectedLifecyclePairIsPresent,
           ResyncValidatedLifecycleRootDirectoryV3,
           SyncedSelectedLifecycleStateSlot,
           SyncedSelectedLifecycleSnapshotV3,
           SelectedLifecycleStateSlot,
           SelectedLifecycleSnapshotV3,
           LifecycleStateSlot, LifecycleSnapshotDigest
  <1>3. CASE PersistFreshRequesterEpoch
    BY <1>3, IsaT(600)
       DEF TypedRolloverSafetyInvariant,
           RootAnchoredLifecycleV3Invariant,
           PersistFreshRequesterEpoch,
           SyncedSelectedLifecycleStateSlot,
           SyncedSelectedLifecycleSnapshotV3,
           SelectedLifecycleStateSlot,
           SelectedLifecycleSnapshotV3,
           LifecycleStateSlot, LifecycleRootV3,
           LifecycleSnapshotV3, LifecycleSnapshotDigest
  <1>4. CASE AuthenticateServerClosePrefix
    BY <1>4, IsaT(600)
       DEF TypedRolloverSafetyInvariant,
           RootAnchoredLifecycleV3Invariant,
           AuthenticateServerClosePrefix,
           SyncedSelectedLifecycleStateSlot,
           SyncedSelectedLifecycleSnapshotV3,
           SelectedLifecycleStateSlot,
           SelectedLifecycleSnapshotV3,
           LifecycleStateSlot, LifecycleRootV3,
           LifecycleSnapshotV3, LifecycleSnapshotDigest
  <1>5. CASE TerminalizeRequestGate
    BY <1>5, IsaT(600)
       DEF TypedRolloverSafetyInvariant,
           RootAnchoredLifecycleV3Invariant,
           TerminalizeRequestGate,
           SyncedSelectedLifecycleStateSlot,
           SyncedSelectedLifecycleSnapshotV3,
           SelectedLifecycleStateSlot,
           SelectedLifecycleSnapshotV3,
           LifecycleStateSlot, LifecycleRootV3,
           LifecycleSnapshotV3, LifecycleSnapshotDigest
  <1>6. CASE CommitSuccessorLifecycleRootV3
    BY <1>6, IsaT(600)
       DEF TypedRolloverSafetyInvariant,
           RootAnchoredLifecycleV3Invariant,
           SyncedRootSelectedLifecyclePairMatches,
           RootSelectedLifecyclePairMatches,
           RootSelectedLifecyclePairIsPresent,
           CommitSuccessorLifecycleRootV3,
           SyncedSelectedLifecycleStateSlot,
           SyncedSelectedLifecycleSnapshotV3,
           SelectedLifecycleStateSlot,
           SelectedLifecycleSnapshotV3,
           LifecycleStateSlot, LifecycleSnapshotDigest
  <1>7. CASE ~(CommitInitialLifecycleRootV3
                 \/ ResyncValidatedLifecycleRootDirectoryV3
                 \/ PersistFreshRequesterEpoch
                 \/ AuthenticateServerClosePrefix
                 \/ TerminalizeRequestGate
                 \/ CommitSuccessorLifecycleRootV3)
    BY <1>7, IsaT(900)
       DEF Next, CreateServiceTransportOwnerPair, ValidateFinality,
           CloseWorkerIngress, ClearOneWorkerExactOutput,
           BuildImmediateSuccessor, SealAppliedHeightOutputHandoff,
           RejectLateExactOutputEnqueue,
           PresentForeignOwnerReceiptCandidate,
           PresentPredecessorContextMismatchCandidate,
           PresentPredecessorArtifactMismatchCandidate,
           PresentWrongImmediateSuccessorCandidate,
           RejectForeignOwnerReceipt,
           RejectPredecessorContextMismatch,
           RejectPredecessorArtifactMismatch,
           RejectWrongImmediateSuccessor, RetainExactHandoffReceipt,
           PublishInitialLifecycleStateSlotV3,
           SyncInitialLifecycleStateDirectoryV3,
           CrashAfterBootstrapStateReplacement,
           CrashAfterBootstrapStatePublication,
           ValidateBootstrapLifecycleCandidateV3,
           ValidateBootstrapLifecycleWithoutCandidateV3,
           ReplaceInitialLifecycleRootV3,
           CrashAfterBootstrapRootReplacement,
           ValidateRootSelectedLifecycleV3,
           RejectLifecycleRootShapeMismatchV3,
           RejectLifecycleSelectedStateMissingV3,
           RejectLifecycleGenerationHashMismatchV3,
           RejectLifecycleSemanticValidationFailureV3,
           RejectInvalidLifecycleStartupV3,
           ResyncValidatedLifecycleStateDirectoryV3,
           CrashDuringValidatedRestartBeforeRootResyncV3,
           CleanupValidatedLifecycleArtifactsV3,
           CompleteBootstrapRestartWithoutCandidateV3,
           PublishFreshRequesterEpoch, CompleteRequesterEpoch,
           ReopenRequesterEpochAllocator,
           CrashAfterRequesterEpochPersistence,
           RestoreRequesterEpochCounterAfterCrash,
           RejectRequesterEpochOverflow,
           RejectActiveOrdinaryRollover, RejectSameRosterFullTable,
           TerminalizeTransfer, TerminalizeFlush,
           PublishSuccessorLifecycleStateSlotV3,
           PublishSuccessorLifecycleStateSlotV3WithAuthority,
           FailSuccessorLifecycleStateSlotV3Persistence,
           CrashBeforeLifecycleStateSlotV3Publication,
           CrashAfterLifecycleStateReplacement,
           SyncSuccessorLifecycleStateDirectoryV3,
           CrashAfterLifecycleStateSlotV3Publication,
           FailSuccessorLifecycleRootV3Persistence,
           ReplaceSuccessorLifecycleRootV3,
           CrashAfterLifecycleRootReplacement,
           RecoverPredecessorLifecycleV3,
           CleanupCommittedLifecyclePredecessorV3,
           CrashAfterLifecycleRootV3Commit,
           RestoreSuccessorLifecycleV3AfterCrash,
           PublishCommittedLifecycleV3ToMemory,
           ActivateRestoredLifecycleV3Successor,
           ActivateSameRosterSuccessor,
           RejectServiceGenerationOverflow,
           FailLifecycleRootGenerationExhaustion,
           ObserveLateOldWriterCallback,
           CrashClearedProcessLocalRolloverState,
           SyncedSelectedLifecycleStateSlot,
           SyncedSelectedLifecycleSnapshotV3,
           LifecycleStateSlot, LifecycleSnapshotDigest
  <1> QED BY <1>1, <1>2, <1>3, <1>4, <1>5, <1>6, <1>7

THEOREM MemoryPublicationRequiresCommittedV3RootObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ PublishCommittedLifecycleV3ToMemory
  =>
    /\ DurableSnapshot(state).version = 3
    /\ DurableSnapshot(state).serverStreams = "Empty"
    /\ DurableSnapshot(state).requestGates = "Empty"
    /\ DurableSnapshot(state).serverClosePrefix = 0
    /\ state'.serviceGeneration =
         DurableSnapshot(state).serviceGeneration
    /\ state'.lifecycleCommitPhase = "Published"
PROOF
  BY Isa DEF PublishCommittedLifecycleV3ToMemory,
                 RootCommittedSuccessorAheadOfMemory,
                 LifecycleSnapshotSemanticallyValid

THEOREM OrdinaryRolloverRequiresAuthenticatedTerminalityObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ PublishSuccessorLifecycleStateSlotV3
  /\ state'.pendingRolloverAuthority = "AuthenticatedTerminal"
  =>
    AllOldLifecycleTerminal
PROOF
  BY Isa DEF PublishSuccessorLifecycleStateSlotV3,
                 PublishSuccessorLifecycleStateSlotV3WithAuthority,
                 ChangedRosterAuthorityAvailable

THEOREM DurableExactOutputAuthorityMayFenceActiveStateObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ PublishDurableExactOutputSuccessorLifecycleStateSlotV3
  /\ ~AllOldLifecycleTerminal
  =>
    /\ ExactRetainedMergeSidecars
    /\ state.durableJournalValidated
    /\ state'.candidateLifecycleSnapshotV3.serverStreams = "Empty"
    /\ state'.candidateLifecycleSnapshotV3.requestGates = "Empty"
    /\ state'.candidateLifecycleSnapshotV3.serverClosePrefix = 0
PROOF
  BY Isa DEF PublishDurableExactOutputSuccessorLifecycleStateSlotV3,
                 PublishSuccessorLifecycleStateSlotV3WithAuthority,
                 ChangedRosterAuthorityAvailable, LifecycleSnapshotV3

THEOREM ValidatedRestartAuthorityMayFenceActiveStateObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ PublishRestartRestoreSuccessorLifecycleStateSlotV3
  /\ ~AllOldLifecycleTerminal
  =>
    /\ state.durableJournalValidated
    /\ state.validatedRestartObserved
    /\ state.restartFenceAuthorized
    /\ state.receiptStage \in {"Absent", "Lost"}
PROOF
  BY Isa DEF PublishRestartRestoreSuccessorLifecycleStateSlotV3,
                 PublishSuccessorLifecycleStateSlotV3WithAuthority,
                 ChangedRosterAuthorityAvailable

THEOREM ActiveOrdinaryRolloverReturnsCapacityAtomicallyObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ RejectActiveOrdinaryRollover
  =>
    /\ state'.capacityRejected
    /\ LifecycleMemory(state') = LifecycleMemory(state)
    /\ DurableLifecycle(state') = DurableLifecycle(state)
    /\ CandidateLifecycle(state') = CandidateLifecycle(state)
PROOF
  BY Isa DEF RejectActiveOrdinaryRollover, LifecycleMemory,
                 DurableLifecycle, CandidateLifecycle

THEOREM SameRosterFullTableReturnsCapacityAtomicallyObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ RejectSameRosterFullTable
  =>
    /\ state'.capacityRejected
    /\ LifecycleMemory(state') = LifecycleMemory(state)
    /\ DurableLifecycle(state') = DurableLifecycle(state)
    /\ CandidateLifecycle(state') = CandidateLifecycle(state)
PROOF
  BY Isa DEF RejectSameRosterFullTable, LifecycleMemory,
                 DurableLifecycle, CandidateLifecycle

THEOREM ServiceGenerationOverflowReturnsCapacityAtomicallyObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ RejectServiceGenerationOverflow
  =>
    /\ state'.capacityRejected
    /\ LifecycleMemory(state') = LifecycleMemory(state)
    /\ DurableLifecycle(state') = DurableLifecycle(state)
    /\ CandidateLifecycle(state') = CandidateLifecycle(state)
PROOF
  BY Isa DEF RejectServiceGenerationOverflow, LifecycleMemory,
                 DurableLifecycle, CandidateLifecycle

THEOREM RootGenerationExhaustionPoisonsJournalFailAtomicallyObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ FailLifecycleRootGenerationExhaustion
  =>
    /\ state'.restartRequired
    /\ state'.failureReason = "LifecycleRootGenerationOverflow"
    /\ state'.capacityRejected = state.capacityRejected
    /\ LifecycleMemory(state') = LifecycleMemory(state)
    /\ DurableLifecycle(state') = DurableLifecycle(state)
    /\ ~state'.durableJournalValidated
    /\ ~state'.successorActive
PROOF
  BY Isa DEF FailLifecycleRootGenerationExhaustion,
                 CrashClearedProcessLocalRolloverState,
                 LifecycleMemory, DurableLifecycle

THEOREM EpochOverflowReturnsCapacityAtomicallyObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ RejectRequesterEpochOverflow
  =>
    /\ state'.capacityRejected
    /\ LifecycleMemory(state') = LifecycleMemory(state)
    /\ DurableLifecycle(state') = DurableLifecycle(state)
    /\ CandidateLifecycle(state') = CandidateLifecycle(state)
PROOF
  BY Isa DEF RejectRequesterEpochOverflow, LifecycleMemory,
                 DurableLifecycle, CandidateLifecycle

THEOREM CrashBeforeRootCommitRestoresPredecessorObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ RecoverPredecessorLifecycleV3
  =>
    /\ state'.currentRoster = DurableSnapshot(state).roster
    /\ state'.serviceGeneration =
         DurableSnapshot(state).serviceGeneration
    /\ ~state'.candidatePresent
    /\ state'.lifecycleCommitPhase = "Current"
    /\ state'.validatedRestartObserved
    /\ (state'.targetRoster # state'.currentRoster =>
          state'.restartFenceAuthorized)
PROOF
  BY Isa DEF RecoverPredecessorLifecycleV3,
                 LifecycleMemoryMatchesDurableSnapshotV3,
                 PersistentLifecycleMemoryMatchesSnapshot

THEOREM CrashAfterRootCommitRestoresSuccessorObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ RestoreSuccessorLifecycleV3AfterCrash
  =>
    /\ state'.currentRoster = DurableSnapshot(state).roster
    /\ state'.serviceGeneration =
         DurableSnapshot(state).serviceGeneration
    /\ state'.nextStreamEpoch =
         DurableSnapshot(state).nextStreamEpoch
    /\ state'.lifecycleCommitPhase = "Restored"
    /\ state'.transitionAuthority = "RestartRestore"
    /\ ~state'.successorActive
PROOF
  BY Isa DEF RestoreSuccessorLifecycleV3AfterCrash

THEOREM FreshEpochPersistencePrecedesExactUseObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ PublishFreshRequesterEpoch
  =>
    /\ state.requesterEpochPhase = "Persisted"
    /\ state.pendingStreamEpoch =
         DurableSnapshot(state).nextStreamEpoch
    /\ DurableSnapshot(state).requesterStreamEpoch =
         DurableSnapshot(state).nextStreamEpoch
    /\ state'.activeStreamEpoch =
         DurableSnapshot(state).requesterStreamEpoch
    /\ state'.nextStreamEpoch = state'.activeStreamEpoch
PROOF
  BY Isa DEF PublishFreshRequesterEpoch,
                 RequesterEpochPersistenceAheadOfMemory

THEOREM CrashRestoresExactRequesterIncarnationObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ RestoreRequesterEpochCounterAfterCrash
  =>
    /\ state'.nextStreamEpoch =
         DurableSnapshot(state).nextStreamEpoch
    /\ state'.activeStreamEpoch =
         DurableSnapshot(state).requesterStreamEpoch
    /\ state'.activeStreamEpoch = state'.nextStreamEpoch
    /\ state'.pendingStreamEpoch = 0
    /\ state'.requesterEpochReplacementRestored
PROOF
  BY Isa DEF RestoreRequesterEpochCounterAfterCrash

THEOREM SameRosterPreservesTransportWithoutGenerationRollObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ ActivateSameRosterSuccessor
  =>
    /\ LifecycleMemory(state') = LifecycleMemory(state)
    /\ DurableLifecycle(state') = DurableLifecycle(state)
    /\ CandidateLifecycle(state') = CandidateLifecycle(state)
    /\ state'.serviceGeneration = state.serviceGeneration
    /\ state'.retryableChunk = 1
PROOF
  BY Isa DEF ActivateSameRosterSuccessor,
                 PredecessorTransportOwnershipOpen,
                 TypedRolloverSafetyInvariant,
                 LifecycleCommitPhaseInvariant,
                 SameRosterTransportPreservationInvariant,
                 LifecycleMemory, DurableLifecycle, CandidateLifecycle

THEOREM ForcedFenceCannotForgeAuthenticatedClosePrefixObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ (PublishCommittedLifecycleV3ToMemory
       \/ RestoreSuccessorLifecycleV3AfterCrash)
  /\ state'.transitionAuthority \in
       {"DurableExactOutput", "RestartRestore"}
  =>
    /\ state'.recordedRetiredClosePrefix =
         state.recordedRetiredClosePrefix
    /\ state'.recordedRetiredClosePrefix <=
         state'.authenticatedCloseHistory
PROOF
  <1>1. CASE PublishCommittedLifecycleV3ToMemory
    BY <1>1, IsaT(240)
       DEF TypedRolloverSafetyInvariant,
           NoForgedAuthenticatedClosePrefixInvariant,
           PublishCommittedLifecycleV3ToMemory
  <1>2. CASE RestoreSuccessorLifecycleV3AfterCrash
    BY <1>2, IsaT(240)
       DEF TypedRolloverSafetyInvariant,
           NoForgedAuthenticatedClosePrefixInvariant,
           RestoreSuccessorLifecycleV3AfterCrash
  <1> QED BY <1>1, <1>2

THEOREM LateOldCallbackCannotMutateSuccessorObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ ObserveLateOldWriterCallback
  =>
    /\ LifecycleMemory(state') = LifecycleMemory(state)
    /\ DurableLifecycle(state') = DurableLifecycle(state)
    /\ CandidateLifecycle(state') = CandidateLifecycle(state)
PROOF
  BY Isa DEF ObserveLateOldWriterCallback, LifecycleMemory,
                 DurableLifecycle, CandidateLifecycle

THEOREM TypedRolloverSpecAlwaysSafeObligation ==
  TypedRolloverSpec => []TypedRolloverSafetyInvariant
PROOF
  <1>1. Init => TypedRolloverSafetyInvariant
    BY TypedRolloverInitEstablishesSafetyObligation
  <1>2. /\ TypedRolloverSafetyInvariant
         /\ [Next]_typedRolloverVars
         => TypedRolloverSafetyInvariant'
    <2>1. CASE Next
      BY <2>1, TypedRolloverNextPreservesSafetyObligation
    <2>2. CASE UNCHANGED typedRolloverVars
      BY <2>2, IsaT(120)
         DEF typedRolloverVars, TypedRolloverSafetyInvariant
    <2> QED BY <2>1, <2>2
  <1>3. TypedRolloverSafetyInvariant /\ [][Next]_typedRolloverVars
         => []TypedRolloverSafetyInvariant
    BY <1>2
  <1> QED BY <1>1, <1>3, PTL DEF TypedRolloverSpec

(***************************************************************************
Responsive durable-output corridor.

Every stage is goal-inclusive.  Consequently an already-published successor
cannot be pulled back into an earlier source, while a pending stage names one
continuously enabled reviewed action.  Stages 3 and 4 are deliberately
separate: they account for workerOutstanding 2 -> 1 and 1 -> 0 using two
distinct applications of weak fairness for ClearOneWorkerExactOutput.
***************************************************************************)
LOCAL DurableOutputGoal ==
  DurableExactOutputSuccessorActiveWithoutRestart

LOCAL DurableOutputCorridorBase ==
  /\ TypedRolloverSafetyInvariant
  /\ state.startupMode = "LiveProcess"
  /\ state.startupValidationFault = "None"
  /\ state.requesterEpochPhase = "Idle"
  /\ state.targetRoster # state.baselineRoster
  /\ state.durableJournalValidated
  /\ NoRolloverFailure
  /\ \/ DurableOutputGoal
     \/ /\ state.currentRoster = state.baselineRoster
        /\ state.transitionAuthority = "None"
        /\ ~state.successorActive
        /\ \/ /\ state.constructionParent = NoIdentity
               /\ state.constructionSuccessor = NoIdentity
           \/ ExactSuccessorConstruction
        /\ state.receiptStage \in {"Absent", "Minted", "Retained"}
        /\ ChangedRosterReplacementNeeded
        /\ state.serviceGeneration < ServiceGenerationLimit
        /\ (state.lifecycleCommitPhase = "Current" =>
              /\ state.durableLifecycleRootV3.rootGeneration <
                   RootGenerationLimit
              /\ LifecycleJournalReady(state)
              /\ LifecycleMemoryMatchesDurableSnapshotV3)

LOCAL DurableOutputStage0 ==
  \/ DurableOutputGoal
  \/ /\ DurableOutputCorridorBase
     /\ state.finalityValidated

LOCAL DurableOutputStage1 ==
  \/ DurableOutputGoal
  \/ /\ DurableOutputCorridorBase
     /\ state.finalityValidated
     /\ ExactServiceTransportOwnerPair

LOCAL DurableOutputStage2 ==
  \/ DurableOutputGoal
  \/ /\ DurableOutputCorridorBase
     /\ state.finalityValidated
     /\ ExactServiceTransportOwnerPair
     /\ state.workerIngressClosed

LOCAL DurableOutputStage3 ==
  \/ DurableOutputGoal
  \/ /\ DurableOutputCorridorBase
     /\ state.finalityValidated
     /\ ExactServiceTransportOwnerPair
     /\ state.workerIngressClosed
     /\ state.workerOutstanding \in 0..1

LOCAL DurableOutputStage4 ==
  \/ DurableOutputGoal
  \/ /\ DurableOutputCorridorBase
     /\ state.finalityValidated
     /\ ExactServiceTransportOwnerPair
     /\ state.workerIngressClosed
     /\ state.workerOutstanding = 0

LOCAL DurableOutputStage5 ==
  \/ DurableOutputGoal
  \/ /\ DurableOutputCorridorBase
     /\ state.finalityValidated
     /\ ExactServiceTransportOwnerPair
     /\ state.workerIngressClosed
     /\ state.workerOutstanding = 0
     /\ ExactSuccessorConstruction

LOCAL DurableOutputStage6 ==
  \/ DurableOutputGoal
  \/ /\ DurableOutputCorridorBase
     /\ state.finalityValidated
     /\ ExactServiceTransportOwnerPair
     /\ state.workerIngressClosed
     /\ state.workerOutstanding = 0
     /\ ExactSuccessorConstruction
     /\ state.receiptStage \in {"Minted", "Retained"}

LOCAL DurableOutputStage7 ==
  \/ DurableOutputGoal
  \/ /\ DurableOutputCorridorBase
     /\ state.receiptStage = "Retained"
     /\ ExactRetainedMergeSidecars
     /\ state.lifecycleCommitPhase = "Current"

LOCAL DurableOutputStage8 ==
  \/ DurableOutputGoal
  \/ /\ DurableOutputCorridorBase
     /\ state.lifecycleCommitPhase = "StateSlotReplaced"
     /\ state.pendingRolloverAuthority = "DurableExactOutput"

LOCAL DurableOutputStage9 ==
  \/ DurableOutputGoal
  \/ /\ DurableOutputCorridorBase
     /\ state.lifecycleCommitPhase = "StateSlotPublished"
     /\ state.pendingRolloverAuthority = "DurableExactOutput"

LOCAL DurableOutputStage10 ==
  \/ DurableOutputGoal
  \/ /\ DurableOutputCorridorBase
     /\ state.lifecycleCommitPhase = "RootReplaced"
     /\ state.pendingRolloverAuthority = "DurableExactOutput"

LOCAL DurableOutputStage11 ==
  \/ DurableOutputGoal
  \/ /\ DurableOutputCorridorBase
     /\ state.lifecycleCommitPhase = "RootCommitted"
     /\ state.pendingRolloverAuthority = "DurableExactOutput"

LOCAL DurableOutputPending0 ==
  DurableOutputStage0 /\ ~DurableOutputStage1
LOCAL DurableOutputPending1 ==
  DurableOutputStage1 /\ ~DurableOutputStage2
LOCAL DurableOutputPending2 ==
  DurableOutputStage2 /\ ~DurableOutputStage3
LOCAL DurableOutputPending3 ==
  DurableOutputStage3 /\ ~DurableOutputStage4
LOCAL DurableOutputPending4 ==
  DurableOutputStage4 /\ ~DurableOutputStage5
LOCAL DurableOutputPending5 ==
  DurableOutputStage5 /\ ~DurableOutputStage6
LOCAL DurableOutputPending6 ==
  DurableOutputStage6 /\ ~DurableOutputStage7
LOCAL DurableOutputPending7 ==
  DurableOutputStage7 /\ ~DurableOutputStage8
LOCAL DurableOutputPending8 ==
  DurableOutputStage8 /\ ~DurableOutputStage9
LOCAL DurableOutputPending9 ==
  DurableOutputStage9 /\ ~DurableOutputStage10
LOCAL DurableOutputPending10 ==
  DurableOutputStage10 /\ ~DurableOutputStage11
LOCAL DurableOutputPending11 ==
  DurableOutputStage11 /\ ~DurableOutputGoal

THEOREM ResponsiveDurableOutputInitEstablishesCorridorBase ==
  ResponsiveDurableExactOutputInit => DurableOutputCorridorBase
BY TypedRolloverInitEstablishesSafetyObligation, IsaT(600)
   DEF ResponsiveDurableExactOutputInit,
       DurableOutputCorridorBase, DurableOutputGoal,
       DurableExactOutputSuccessorActiveWithoutRestart,
       NoRolloverFailure, ChangedRosterReplacementNeeded,
       CompactionCauseMatchesGeometry, LifecycleJournalReady,
       LifecycleMemoryMatchesDurableSnapshotV3,
       PersistentLifecycleMemoryMatchesSnapshot,
       RootSelectedLifecyclePairMatches,
       RootSelectedLifecyclePairIsPresent,
       LifecycleSnapshotSemanticallyValid,
       LifecycleStateDirectoryIsSynced,
       LifecycleRootDirectoryIsSynced, DurableSnapshot,
       SelectedLifecycleSnapshotV3, SelectedLifecycleStateSlot,
       Init, InitialLifecycleSnapshotV3, LiveLifecycleSnapshotV3,
       InitialLifecycleStateSlotsV3, LifecycleSnapshotV3,
       LifecycleRootV3, LifecycleSnapshotDigest,
       LifecycleStateSlot, NoLifecycleSnapshot

THEOREM ResponsiveDurableOutputStepPreservesCorridorBase ==
  /\ DurableOutputCorridorBase
  /\ [ResponsiveDurableExactOutputNext]_typedRolloverVars
  => DurableOutputCorridorBase'
PROOF
  <1>1. CASE ResponsiveDurableExactOutputNext
    <2>1. TypedRolloverSafetyInvariant'
      BY <1>1, TypedRolloverNextPreservesSafetyObligation
         DEF ResponsiveDurableExactOutputNext, Next
    <2>2. DurableOutputCorridorBase'
      BY <1>1, <2>1, IsaT(900)
         DEF DurableOutputCorridorBase, DurableOutputGoal,
             DurableExactOutputSuccessorActiveWithoutRestart,
             NoRolloverFailure, ChangedRosterReplacementNeeded,
             ExactSuccessorConstruction,
             CompactionCauseMatchesGeometry, LifecycleJournalReady,
             LifecycleMemoryMatchesDurableSnapshotV3,
             PersistentLifecycleMemoryMatchesSnapshot,
             RootSelectedLifecyclePairMatches,
             RootSelectedLifecyclePairIsPresent,
             LifecycleSnapshotSemanticallyValid,
             LifecycleStateDirectoryIsSynced,
             LifecycleRootDirectoryIsSynced, DurableSnapshot,
             SelectedLifecycleSnapshotV3,
             SelectedLifecycleStateSlot,
             ResponsiveDurableExactOutputNext,
             CreateServiceTransportOwnerPair, ValidateFinality,
             CloseWorkerIngress, ClearOneWorkerExactOutput,
             BuildImmediateSuccessor,
             SealAppliedHeightOutputHandoff,
             RetainExactHandoffReceipt,
             PublishDurableExactOutputSuccessorLifecycleStateSlotV3,
             PublishSuccessorLifecycleStateSlotV3WithAuthority,
             SyncSuccessorLifecycleStateDirectoryV3,
             ReplaceSuccessorLifecycleRootV3,
             CommitSuccessorLifecycleRootV3,
             PublishCommittedLifecycleV3ToMemory,
             LifecycleRootV3, LifecycleSnapshotV3,
             LifecycleSnapshotDigest, LifecycleStateSlot
    <2> QED BY <2>2
  <1>2. CASE UNCHANGED typedRolloverVars
    BY <1>2, IsaT(120)
       DEF typedRolloverVars, DurableOutputCorridorBase
  <1> QED BY <1>1, <1>2

THEOREM ResponsiveDurableOutputAlwaysCorridorBase ==
  ResponsiveDurableExactOutputSpec => []DurableOutputCorridorBase
PROOF
  <1>1. ResponsiveDurableExactOutputInit =>
          DurableOutputCorridorBase
    BY ResponsiveDurableOutputInitEstablishesCorridorBase
  <1>2. /\ DurableOutputCorridorBase
         /\ [ResponsiveDurableExactOutputNext]_typedRolloverVars
         => DurableOutputCorridorBase'
    BY ResponsiveDurableOutputStepPreservesCorridorBase
  <1>3. DurableOutputCorridorBase
         /\ [][ResponsiveDurableExactOutputNext]_typedRolloverVars
         => []DurableOutputCorridorBase
    BY <1>2
  <1> QED BY <1>1, <1>3, PTL
       DEF ResponsiveDurableExactOutputSpec

THEOREM DurableOutputPending0IsNotOrphaned ==
  /\ DurableOutputPending0
  /\ [ResponsiveDurableExactOutputNext]_typedRolloverVars
  => \/ DurableOutputPending0'
     \/ DurableOutputStage1'
BY ResponsiveDurableOutputStepPreservesCorridorBase, IsaT(600)
   DEF DurableOutputPending0, DurableOutputStage0,
       DurableOutputStage1, DurableOutputCorridorBase,
       DurableOutputGoal,
       DurableExactOutputSuccessorActiveWithoutRestart,
       ResponsiveDurableExactOutputNext,
       CreateServiceTransportOwnerPair, ValidateFinality,
       CloseWorkerIngress, ClearOneWorkerExactOutput,
       BuildImmediateSuccessor, SealAppliedHeightOutputHandoff,
       RetainExactHandoffReceipt,
       PublishDurableExactOutputSuccessorLifecycleStateSlotV3,
       PublishSuccessorLifecycleStateSlotV3WithAuthority,
       SyncSuccessorLifecycleStateDirectoryV3,
       ReplaceSuccessorLifecycleRootV3,
       CommitSuccessorLifecycleRootV3,
       PublishCommittedLifecycleV3ToMemory

THEOREM DurableOutputPending0EnablesCreate ==
  DurableOutputPending0 =>
    ENABLED <<CreateServiceTransportOwnerPair>>_typedRolloverVars
BY ExpandENABLED, IsaT(300)
   DEF DurableOutputPending0, DurableOutputStage0,
       DurableOutputStage1, DurableOutputCorridorBase,
       DurableOutputGoal,
       DurableExactOutputSuccessorActiveWithoutRestart,
       TypedRolloverSafetyInvariant,
       ExactServiceTransportOwnerPairInvariant,
       ReceiptLifecycleInvariant, ExactPredecessorReceipt,
       ExactSuccessorConstruction,
       PredecessorTransportOwnershipOpen,
       CreateServiceTransportOwnerPair, typedRolloverVars

THEOREM DurableOutputCreateExitsPending0 ==
  /\ DurableOutputPending0
  /\ <<CreateServiceTransportOwnerPair>>_typedRolloverVars
  => DurableOutputStage1'
BY ResponsiveDurableOutputStepPreservesCorridorBase, IsaT(300)
   DEF DurableOutputPending0, DurableOutputStage0,
       DurableOutputStage1, DurableOutputGoal,
       CreateServiceTransportOwnerPair, typedRolloverVars,
       ResponsiveDurableExactOutputNext

THEOREM DurableOutputStage0LeadsToStage1 ==
  ResponsiveDurableExactOutputSpec =>
    (DurableOutputStage0 ~> DurableOutputStage1)
PROOF
  <1>1. /\ DurableOutputPending0
         /\ [ResponsiveDurableExactOutputNext]_typedRolloverVars
         => \/ DurableOutputPending0'
            \/ DurableOutputStage1'
    BY DurableOutputPending0IsNotOrphaned
  <1>2. DurableOutputPending0 =>
          ENABLED <<CreateServiceTransportOwnerPair>>_typedRolloverVars
    BY DurableOutputPending0EnablesCreate
  <1>3. /\ DurableOutputPending0
         /\ <<CreateServiceTransportOwnerPair>>_typedRolloverVars
         => DurableOutputStage1'
    BY DurableOutputCreateExitsPending0
  <1>4. ResponsiveDurableExactOutputSpec =>
          WF_typedRolloverVars(CreateServiceTransportOwnerPair)
    BY DEF ResponsiveDurableExactOutputSpec
  <1>5. ResponsiveDurableExactOutputSpec =>
          [][ResponsiveDurableExactOutputNext]_typedRolloverVars
    BY DEF ResponsiveDurableExactOutputSpec
  <1>6. ResponsiveDurableExactOutputSpec =>
          [](DurableOutputPending0 /\ ~DurableOutputStage1
             => ENABLED
                  <<CreateServiceTransportOwnerPair>>_typedRolloverVars)
    BY <1>2, PTL
  <1>7. ResponsiveDurableExactOutputSpec =>
          [](DurableOutputPending0 /\ ~DurableOutputStage1
             /\ <<CreateServiceTransportOwnerPair>>_typedRolloverVars
             => DurableOutputStage1')
    BY <1>3, PTL
  <1>8. ResponsiveDurableExactOutputSpec =>
          [](DurableOutputPending0 /\ ~DurableOutputStage1
             /\ [ResponsiveDurableExactOutputNext]_typedRolloverVars
             => \/ DurableOutputStage1'
                \/ DurableOutputPending0')
    BY <1>1, PTL
  <1> QED BY <1>4, <1>5, <1>6, <1>7, <1>8, PTL
       DEF DurableOutputPending0

THEOREM DurableOutputPending1IsNotOrphaned ==
  /\ DurableOutputPending1
  /\ [ResponsiveDurableExactOutputNext]_typedRolloverVars
  => \/ DurableOutputPending1'
     \/ DurableOutputStage2'
BY ResponsiveDurableOutputStepPreservesCorridorBase, IsaT(600)
   DEF DurableOutputPending1, DurableOutputStage1,
       DurableOutputStage2, DurableOutputCorridorBase,
       DurableOutputGoal,
       DurableExactOutputSuccessorActiveWithoutRestart,
       ResponsiveDurableExactOutputNext,
       CreateServiceTransportOwnerPair, ValidateFinality,
       CloseWorkerIngress, ClearOneWorkerExactOutput,
       BuildImmediateSuccessor, SealAppliedHeightOutputHandoff,
       RetainExactHandoffReceipt,
       PublishDurableExactOutputSuccessorLifecycleStateSlotV3,
       PublishSuccessorLifecycleStateSlotV3WithAuthority,
       SyncSuccessorLifecycleStateDirectoryV3,
       ReplaceSuccessorLifecycleRootV3,
       CommitSuccessorLifecycleRootV3,
       PublishCommittedLifecycleV3ToMemory

THEOREM DurableOutputPending1EnablesClose ==
  DurableOutputPending1 =>
    ENABLED <<CloseWorkerIngress>>_typedRolloverVars
BY ExpandENABLED, IsaT(180)
   DEF DurableOutputPending1, DurableOutputStage1,
       DurableOutputStage2, DurableOutputCorridorBase,
       DurableOutputGoal,
       DurableExactOutputSuccessorActiveWithoutRestart,
       NoRolloverFailure, CloseWorkerIngress, typedRolloverVars

THEOREM DurableOutputCloseExitsPending1 ==
  /\ DurableOutputPending1
  /\ <<CloseWorkerIngress>>_typedRolloverVars
  => DurableOutputStage2'
BY ResponsiveDurableOutputStepPreservesCorridorBase, IsaT(180)
   DEF DurableOutputPending1, DurableOutputStage1,
       DurableOutputStage2, DurableOutputGoal,
       CloseWorkerIngress, typedRolloverVars,
       ResponsiveDurableExactOutputNext

THEOREM DurableOutputStage1LeadsToStage2 ==
  ResponsiveDurableExactOutputSpec =>
    (DurableOutputStage1 ~> DurableOutputStage2)
PROOF
  <1>1. /\ DurableOutputPending1
         /\ [ResponsiveDurableExactOutputNext]_typedRolloverVars
         => \/ DurableOutputPending1'
            \/ DurableOutputStage2'
    BY DurableOutputPending1IsNotOrphaned
  <1>2. DurableOutputPending1 =>
          ENABLED <<CloseWorkerIngress>>_typedRolloverVars
    BY DurableOutputPending1EnablesClose
  <1>3. /\ DurableOutputPending1
         /\ <<CloseWorkerIngress>>_typedRolloverVars
         => DurableOutputStage2'
    BY DurableOutputCloseExitsPending1
  <1>4. ResponsiveDurableExactOutputSpec =>
          WF_typedRolloverVars(CloseWorkerIngress)
    BY DEF ResponsiveDurableExactOutputSpec
  <1>5. ResponsiveDurableExactOutputSpec =>
          [][ResponsiveDurableExactOutputNext]_typedRolloverVars
    BY DEF ResponsiveDurableExactOutputSpec
  <1>6. ResponsiveDurableExactOutputSpec =>
          [](DurableOutputPending1 /\ ~DurableOutputStage2
             => ENABLED <<CloseWorkerIngress>>_typedRolloverVars)
    BY <1>2, PTL
  <1>7. ResponsiveDurableExactOutputSpec =>
          [](DurableOutputPending1 /\ ~DurableOutputStage2
             /\ <<CloseWorkerIngress>>_typedRolloverVars
             => DurableOutputStage2')
    BY <1>3, PTL
  <1>8. ResponsiveDurableExactOutputSpec =>
          [](DurableOutputPending1 /\ ~DurableOutputStage2
             /\ [ResponsiveDurableExactOutputNext]_typedRolloverVars
             => \/ DurableOutputStage2'
                \/ DurableOutputPending1')
    BY <1>1, PTL
  <1> QED BY <1>4, <1>5, <1>6, <1>7, <1>8, PTL
       DEF DurableOutputPending1

THEOREM DurableOutputPending2IsExactlyFirstClear ==
  DurableOutputPending2 => state.workerOutstanding = 2
BY IsaT(120)
   DEF DurableOutputPending2, DurableOutputStage2,
       DurableOutputStage3, DurableOutputCorridorBase,
       DurableOutputGoal, TypedRolloverSafetyInvariant,
       TypedRolloverTypeInvariant

THEOREM DurableOutputPending2IsNotOrphaned ==
  /\ DurableOutputPending2
  /\ [ResponsiveDurableExactOutputNext]_typedRolloverVars
  => \/ DurableOutputPending2'
     \/ DurableOutputStage3'
BY ResponsiveDurableOutputStepPreservesCorridorBase,
   DurableOutputPending2IsExactlyFirstClear, IsaT(600)
   DEF DurableOutputPending2, DurableOutputStage2,
       DurableOutputStage3, DurableOutputCorridorBase,
       DurableOutputGoal,
       DurableExactOutputSuccessorActiveWithoutRestart,
       ResponsiveDurableExactOutputNext,
       CreateServiceTransportOwnerPair, ValidateFinality,
       CloseWorkerIngress, ClearOneWorkerExactOutput,
       BuildImmediateSuccessor, SealAppliedHeightOutputHandoff,
       RetainExactHandoffReceipt,
       PublishDurableExactOutputSuccessorLifecycleStateSlotV3,
       PublishSuccessorLifecycleStateSlotV3WithAuthority,
       SyncSuccessorLifecycleStateDirectoryV3,
       ReplaceSuccessorLifecycleRootV3,
       CommitSuccessorLifecycleRootV3,
       PublishCommittedLifecycleV3ToMemory

THEOREM DurableOutputPending2EnablesFirstClear ==
  DurableOutputPending2 =>
    ENABLED <<ClearOneWorkerExactOutput>>_typedRolloverVars
BY DurableOutputPending2IsExactlyFirstClear,
   ExpandENABLED, IsaT(180)
   DEF DurableOutputPending2, DurableOutputStage2,
       DurableOutputCorridorBase, DurableOutputGoal,
       DurableExactOutputSuccessorActiveWithoutRestart,
       NoRolloverFailure, ClearOneWorkerExactOutput,
       typedRolloverVars

THEOREM DurableOutputFirstClearExitsPending2 ==
  /\ DurableOutputPending2
  /\ <<ClearOneWorkerExactOutput>>_typedRolloverVars
  => /\ state'.workerOutstanding = 1
     /\ DurableOutputStage3'
BY ResponsiveDurableOutputStepPreservesCorridorBase,
   DurableOutputPending2IsExactlyFirstClear, IsaT(180)
   DEF DurableOutputPending2, DurableOutputStage2,
       DurableOutputStage3, DurableOutputGoal,
       ClearOneWorkerExactOutput, typedRolloverVars,
       ResponsiveDurableExactOutputNext

THEOREM DurableOutputStage2LeadsToStage3 ==
  ResponsiveDurableExactOutputSpec =>
    (DurableOutputStage2 ~> DurableOutputStage3)
PROOF
  <1>1. /\ DurableOutputPending2
         /\ [ResponsiveDurableExactOutputNext]_typedRolloverVars
         => \/ DurableOutputPending2'
            \/ DurableOutputStage3'
    BY DurableOutputPending2IsNotOrphaned
  <1>2. DurableOutputPending2 =>
          ENABLED <<ClearOneWorkerExactOutput>>_typedRolloverVars
    BY DurableOutputPending2EnablesFirstClear
  <1>3. /\ DurableOutputPending2
         /\ <<ClearOneWorkerExactOutput>>_typedRolloverVars
         => DurableOutputStage3'
    BY DurableOutputFirstClearExitsPending2
  <1>4. ResponsiveDurableExactOutputSpec =>
          WF_typedRolloverVars(ClearOneWorkerExactOutput)
    BY DEF ResponsiveDurableExactOutputSpec
  <1>5. ResponsiveDurableExactOutputSpec =>
          [][ResponsiveDurableExactOutputNext]_typedRolloverVars
    BY DEF ResponsiveDurableExactOutputSpec
  <1>6. ResponsiveDurableExactOutputSpec =>
          [](DurableOutputPending2 /\ ~DurableOutputStage3
             => ENABLED <<ClearOneWorkerExactOutput>>_typedRolloverVars)
    BY <1>2, PTL
  <1>7. ResponsiveDurableExactOutputSpec =>
          [](DurableOutputPending2 /\ ~DurableOutputStage3
             /\ <<ClearOneWorkerExactOutput>>_typedRolloverVars
             => DurableOutputStage3')
    BY <1>3, PTL
  <1>8. ResponsiveDurableExactOutputSpec =>
          [](DurableOutputPending2 /\ ~DurableOutputStage3
             /\ [ResponsiveDurableExactOutputNext]_typedRolloverVars
             => \/ DurableOutputStage3'
                \/ DurableOutputPending2')
    BY <1>1, PTL
  <1> QED BY <1>4, <1>5, <1>6, <1>7, <1>8, PTL
       DEF DurableOutputPending2

THEOREM DurableOutputPending3IsExactlySecondClear ==
  DurableOutputPending3 => state.workerOutstanding = 1
BY IsaT(120)
   DEF DurableOutputPending3, DurableOutputStage3,
       DurableOutputStage4, DurableOutputCorridorBase,
       DurableOutputGoal, TypedRolloverSafetyInvariant,
       TypedRolloverTypeInvariant

THEOREM DurableOutputPending3IsNotOrphaned ==
  /\ DurableOutputPending3
  /\ [ResponsiveDurableExactOutputNext]_typedRolloverVars
  => \/ DurableOutputPending3'
     \/ DurableOutputStage4'
BY ResponsiveDurableOutputStepPreservesCorridorBase,
   DurableOutputPending3IsExactlySecondClear, IsaT(600)
   DEF DurableOutputPending3, DurableOutputStage3,
       DurableOutputStage4, DurableOutputCorridorBase,
       DurableOutputGoal,
       DurableExactOutputSuccessorActiveWithoutRestart,
       ResponsiveDurableExactOutputNext,
       CreateServiceTransportOwnerPair, ValidateFinality,
       CloseWorkerIngress, ClearOneWorkerExactOutput,
       BuildImmediateSuccessor, SealAppliedHeightOutputHandoff,
       RetainExactHandoffReceipt,
       PublishDurableExactOutputSuccessorLifecycleStateSlotV3,
       PublishSuccessorLifecycleStateSlotV3WithAuthority,
       SyncSuccessorLifecycleStateDirectoryV3,
       ReplaceSuccessorLifecycleRootV3,
       CommitSuccessorLifecycleRootV3,
       PublishCommittedLifecycleV3ToMemory

THEOREM DurableOutputPending3EnablesSecondClear ==
  DurableOutputPending3 =>
    ENABLED <<ClearOneWorkerExactOutput>>_typedRolloverVars
BY DurableOutputPending3IsExactlySecondClear,
   ExpandENABLED, IsaT(180)
   DEF DurableOutputPending3, DurableOutputStage3,
       DurableOutputCorridorBase, DurableOutputGoal,
       DurableExactOutputSuccessorActiveWithoutRestart,
       NoRolloverFailure, ClearOneWorkerExactOutput,
       typedRolloverVars

THEOREM DurableOutputSecondClearExitsPending3 ==
  /\ DurableOutputPending3
  /\ <<ClearOneWorkerExactOutput>>_typedRolloverVars
  => /\ state'.workerOutstanding = 0
     /\ DurableOutputStage4'
BY ResponsiveDurableOutputStepPreservesCorridorBase,
   DurableOutputPending3IsExactlySecondClear, IsaT(180)
   DEF DurableOutputPending3, DurableOutputStage3,
       DurableOutputStage4, DurableOutputGoal,
       ClearOneWorkerExactOutput, typedRolloverVars,
       ResponsiveDurableExactOutputNext

THEOREM DurableOutputStage3LeadsToStage4 ==
  ResponsiveDurableExactOutputSpec =>
    (DurableOutputStage3 ~> DurableOutputStage4)
PROOF
  <1>1. /\ DurableOutputPending3
         /\ [ResponsiveDurableExactOutputNext]_typedRolloverVars
         => \/ DurableOutputPending3'
            \/ DurableOutputStage4'
    BY DurableOutputPending3IsNotOrphaned
  <1>2. DurableOutputPending3 =>
          ENABLED <<ClearOneWorkerExactOutput>>_typedRolloverVars
    BY DurableOutputPending3EnablesSecondClear
  <1>3. /\ DurableOutputPending3
         /\ <<ClearOneWorkerExactOutput>>_typedRolloverVars
         => DurableOutputStage4'
    BY DurableOutputSecondClearExitsPending3
  <1>4. ResponsiveDurableExactOutputSpec =>
          WF_typedRolloverVars(ClearOneWorkerExactOutput)
    BY DEF ResponsiveDurableExactOutputSpec
  <1>5. ResponsiveDurableExactOutputSpec =>
          [][ResponsiveDurableExactOutputNext]_typedRolloverVars
    BY DEF ResponsiveDurableExactOutputSpec
  <1>6. ResponsiveDurableExactOutputSpec =>
          [](DurableOutputPending3 /\ ~DurableOutputStage4
             => ENABLED <<ClearOneWorkerExactOutput>>_typedRolloverVars)
    BY <1>2, PTL
  <1>7. ResponsiveDurableExactOutputSpec =>
          [](DurableOutputPending3 /\ ~DurableOutputStage4
             /\ <<ClearOneWorkerExactOutput>>_typedRolloverVars
             => DurableOutputStage4')
    BY <1>3, PTL
  <1>8. ResponsiveDurableExactOutputSpec =>
          [](DurableOutputPending3 /\ ~DurableOutputStage4
             /\ [ResponsiveDurableExactOutputNext]_typedRolloverVars
             => \/ DurableOutputStage4'
                \/ DurableOutputPending3')
    BY <1>1, PTL
  <1> QED BY <1>4, <1>5, <1>6, <1>7, <1>8, PTL
       DEF DurableOutputPending3

THEOREM DurableOutputPending4IsNotOrphaned ==
  /\ DurableOutputPending4
  /\ [ResponsiveDurableExactOutputNext]_typedRolloverVars
  => \/ DurableOutputPending4'
     \/ DurableOutputStage5'
BY ResponsiveDurableOutputStepPreservesCorridorBase, IsaT(600)
   DEF DurableOutputPending4, DurableOutputStage4,
       DurableOutputStage5, DurableOutputCorridorBase,
       DurableOutputGoal, ExactSuccessorConstruction,
       ResponsiveDurableExactOutputNext,
       CreateServiceTransportOwnerPair, ValidateFinality,
       CloseWorkerIngress, ClearOneWorkerExactOutput,
       BuildImmediateSuccessor, SealAppliedHeightOutputHandoff,
       RetainExactHandoffReceipt,
       PublishDurableExactOutputSuccessorLifecycleStateSlotV3,
       PublishSuccessorLifecycleStateSlotV3WithAuthority,
       SyncSuccessorLifecycleStateDirectoryV3,
       ReplaceSuccessorLifecycleRootV3,
       CommitSuccessorLifecycleRootV3,
       PublishCommittedLifecycleV3ToMemory

THEOREM DurableOutputPending4EnablesBuild ==
  DurableOutputPending4 =>
    ENABLED <<BuildImmediateSuccessor>>_typedRolloverVars
BY ExpandENABLED, IsaT(240)
   DEF DurableOutputPending4, DurableOutputStage4,
       DurableOutputStage5, DurableOutputCorridorBase,
       DurableOutputGoal, ExactSuccessorConstruction,
       NoRolloverFailure, BuildImmediateSuccessor,
       typedRolloverVars

THEOREM DurableOutputBuildExitsPending4 ==
  /\ DurableOutputPending4
  /\ <<BuildImmediateSuccessor>>_typedRolloverVars
  => DurableOutputStage5'
BY ResponsiveDurableOutputStepPreservesCorridorBase, IsaT(240)
   DEF DurableOutputPending4, DurableOutputStage4,
       DurableOutputStage5, DurableOutputGoal,
       ExactSuccessorConstruction, BuildImmediateSuccessor,
       typedRolloverVars, ResponsiveDurableExactOutputNext

THEOREM DurableOutputStage4LeadsToStage5 ==
  ResponsiveDurableExactOutputSpec =>
    (DurableOutputStage4 ~> DurableOutputStage5)
PROOF
  <1>1. /\ DurableOutputPending4
         /\ [ResponsiveDurableExactOutputNext]_typedRolloverVars
         => \/ DurableOutputPending4'
            \/ DurableOutputStage5'
    BY DurableOutputPending4IsNotOrphaned
  <1>2. DurableOutputPending4 =>
          ENABLED <<BuildImmediateSuccessor>>_typedRolloverVars
    BY DurableOutputPending4EnablesBuild
  <1>3. /\ DurableOutputPending4
         /\ <<BuildImmediateSuccessor>>_typedRolloverVars
         => DurableOutputStage5'
    BY DurableOutputBuildExitsPending4
  <1>4. ResponsiveDurableExactOutputSpec =>
          WF_typedRolloverVars(BuildImmediateSuccessor)
    BY DEF ResponsiveDurableExactOutputSpec
  <1>5. ResponsiveDurableExactOutputSpec =>
          [][ResponsiveDurableExactOutputNext]_typedRolloverVars
    BY DEF ResponsiveDurableExactOutputSpec
  <1>6. ResponsiveDurableExactOutputSpec =>
          [](DurableOutputPending4 /\ ~DurableOutputStage5
             => ENABLED <<BuildImmediateSuccessor>>_typedRolloverVars)
    BY <1>2, PTL
  <1>7. ResponsiveDurableExactOutputSpec =>
          [](DurableOutputPending4 /\ ~DurableOutputStage5
             /\ <<BuildImmediateSuccessor>>_typedRolloverVars
             => DurableOutputStage5')
    BY <1>3, PTL
  <1>8. ResponsiveDurableExactOutputSpec =>
          [](DurableOutputPending4 /\ ~DurableOutputStage5
             /\ [ResponsiveDurableExactOutputNext]_typedRolloverVars
             => \/ DurableOutputStage5'
                \/ DurableOutputPending4')
    BY <1>1, PTL
  <1> QED BY <1>4, <1>5, <1>6, <1>7, <1>8, PTL
       DEF DurableOutputPending4

THEOREM DurableOutputPending5IsNotOrphaned ==
  /\ DurableOutputPending5
  /\ [ResponsiveDurableExactOutputNext]_typedRolloverVars
  => \/ DurableOutputPending5'
     \/ DurableOutputStage6'
BY ResponsiveDurableOutputStepPreservesCorridorBase, IsaT(600)
   DEF DurableOutputPending5, DurableOutputStage5,
       DurableOutputStage6, DurableOutputCorridorBase,
       DurableOutputGoal, ExactSuccessorConstruction,
       ResponsiveDurableExactOutputNext,
       CreateServiceTransportOwnerPair, ValidateFinality,
       CloseWorkerIngress, ClearOneWorkerExactOutput,
       BuildImmediateSuccessor, SealAppliedHeightOutputHandoff,
       RetainExactHandoffReceipt,
       PublishDurableExactOutputSuccessorLifecycleStateSlotV3,
       PublishSuccessorLifecycleStateSlotV3WithAuthority,
       SyncSuccessorLifecycleStateDirectoryV3,
       ReplaceSuccessorLifecycleRootV3,
       CommitSuccessorLifecycleRootV3,
       PublishCommittedLifecycleV3ToMemory

THEOREM DurableOutputPending5EnablesSeal ==
  DurableOutputPending5 =>
    ENABLED <<SealAppliedHeightOutputHandoff>>_typedRolloverVars
BY ExpandENABLED, IsaT(300)
   DEF DurableOutputPending5, DurableOutputStage5,
       DurableOutputStage6, DurableOutputCorridorBase,
       DurableOutputGoal, ExactSuccessorConstruction,
       ExactServiceTransportOwnerPair,
       NoRolloverFailure, SealAppliedHeightOutputHandoff,
       PredecessorTransportOwnershipOpen, typedRolloverVars

THEOREM DurableOutputSealExitsPending5 ==
  /\ DurableOutputPending5
  /\ <<SealAppliedHeightOutputHandoff>>_typedRolloverVars
  => DurableOutputStage6'
BY ResponsiveDurableOutputStepPreservesCorridorBase, IsaT(240)
   DEF DurableOutputPending5, DurableOutputStage5,
       DurableOutputStage6, DurableOutputGoal,
       ExactSuccessorConstruction, ExactServiceTransportOwnerPair,
       SealAppliedHeightOutputHandoff, typedRolloverVars,
       ResponsiveDurableExactOutputNext

THEOREM DurableOutputStage5LeadsToStage6 ==
  ResponsiveDurableExactOutputSpec =>
    (DurableOutputStage5 ~> DurableOutputStage6)
PROOF
  <1>1. /\ DurableOutputPending5
         /\ [ResponsiveDurableExactOutputNext]_typedRolloverVars
         => \/ DurableOutputPending5'
            \/ DurableOutputStage6'
    BY DurableOutputPending5IsNotOrphaned
  <1>2. DurableOutputPending5 =>
          ENABLED <<SealAppliedHeightOutputHandoff>>_typedRolloverVars
    BY DurableOutputPending5EnablesSeal
  <1>3. /\ DurableOutputPending5
         /\ <<SealAppliedHeightOutputHandoff>>_typedRolloverVars
         => DurableOutputStage6'
    BY DurableOutputSealExitsPending5
  <1>4. ResponsiveDurableExactOutputSpec =>
          WF_typedRolloverVars(SealAppliedHeightOutputHandoff)
    BY DEF ResponsiveDurableExactOutputSpec
  <1>5. ResponsiveDurableExactOutputSpec =>
          [][ResponsiveDurableExactOutputNext]_typedRolloverVars
    BY DEF ResponsiveDurableExactOutputSpec
  <1>6. ResponsiveDurableExactOutputSpec =>
          [](DurableOutputPending5 /\ ~DurableOutputStage6
             => ENABLED
                  <<SealAppliedHeightOutputHandoff>>_typedRolloverVars)
    BY <1>2, PTL
  <1>7. ResponsiveDurableExactOutputSpec =>
          [](DurableOutputPending5 /\ ~DurableOutputStage6
             /\ <<SealAppliedHeightOutputHandoff>>_typedRolloverVars
             => DurableOutputStage6')
    BY <1>3, PTL
  <1>8. ResponsiveDurableExactOutputSpec =>
          [](DurableOutputPending5 /\ ~DurableOutputStage6
             /\ [ResponsiveDurableExactOutputNext]_typedRolloverVars
             => \/ DurableOutputStage6'
                \/ DurableOutputPending5')
    BY <1>1, PTL
  <1> QED BY <1>4, <1>5, <1>6, <1>7, <1>8, PTL
       DEF DurableOutputPending5

THEOREM DurableOutputPending6IsNotOrphaned ==
  /\ DurableOutputPending6
  /\ [ResponsiveDurableExactOutputNext]_typedRolloverVars
  => \/ DurableOutputPending6'
     \/ DurableOutputStage7'
BY ResponsiveDurableOutputStepPreservesCorridorBase, IsaT(600)
   DEF DurableOutputPending6, DurableOutputStage6,
       DurableOutputStage7, DurableOutputCorridorBase,
       DurableOutputGoal, ExactSuccessorConstruction,
       ExactRetainedMergeSidecars, ExactPredecessorReceipt,
       ExactServiceTransportOwnerPair,
       ResponsiveDurableExactOutputNext,
       CreateServiceTransportOwnerPair, ValidateFinality,
       CloseWorkerIngress, ClearOneWorkerExactOutput,
       BuildImmediateSuccessor, SealAppliedHeightOutputHandoff,
       RetainExactHandoffReceipt,
       PublishDurableExactOutputSuccessorLifecycleStateSlotV3,
       PublishSuccessorLifecycleStateSlotV3WithAuthority,
       SyncSuccessorLifecycleStateDirectoryV3,
       ReplaceSuccessorLifecycleRootV3,
       CommitSuccessorLifecycleRootV3,
       PublishCommittedLifecycleV3ToMemory

THEOREM DurableOutputPending6EnablesRetain ==
  DurableOutputPending6 =>
    ENABLED <<RetainExactHandoffReceipt>>_typedRolloverVars
BY ExpandENABLED, IsaT(300)
   DEF DurableOutputPending6, DurableOutputStage6,
       DurableOutputStage7, DurableOutputCorridorBase,
       DurableOutputGoal, ExactSuccessorConstruction,
       ExactPredecessorReceipt, ExactServiceTransportOwnerPair,
       NoRolloverFailure, RetainExactHandoffReceipt,
       typedRolloverVars

THEOREM DurableOutputRetainExitsPending6 ==
  /\ DurableOutputPending6
  /\ <<RetainExactHandoffReceipt>>_typedRolloverVars
  => DurableOutputStage7'
BY ResponsiveDurableOutputStepPreservesCorridorBase, IsaT(300)
   DEF DurableOutputPending6, DurableOutputStage6,
       DurableOutputStage7, DurableOutputGoal,
       ExactRetainedMergeSidecars, ExactPredecessorReceipt,
       ExactServiceTransportOwnerPair, ExactSuccessorConstruction,
       RetainExactHandoffReceipt, typedRolloverVars,
       ResponsiveDurableExactOutputNext

THEOREM DurableOutputStage6LeadsToStage7 ==
  ResponsiveDurableExactOutputSpec =>
    (DurableOutputStage6 ~> DurableOutputStage7)
PROOF
  <1>1. /\ DurableOutputPending6
         /\ [ResponsiveDurableExactOutputNext]_typedRolloverVars
         => \/ DurableOutputPending6'
            \/ DurableOutputStage7'
    BY DurableOutputPending6IsNotOrphaned
  <1>2. DurableOutputPending6 =>
          ENABLED <<RetainExactHandoffReceipt>>_typedRolloverVars
    BY DurableOutputPending6EnablesRetain
  <1>3. /\ DurableOutputPending6
         /\ <<RetainExactHandoffReceipt>>_typedRolloverVars
         => DurableOutputStage7'
    BY DurableOutputRetainExitsPending6
  <1>4. ResponsiveDurableExactOutputSpec =>
          WF_typedRolloverVars(RetainExactHandoffReceipt)
    BY DEF ResponsiveDurableExactOutputSpec
  <1>5. ResponsiveDurableExactOutputSpec =>
          [][ResponsiveDurableExactOutputNext]_typedRolloverVars
    BY DEF ResponsiveDurableExactOutputSpec
  <1>6. ResponsiveDurableExactOutputSpec =>
          [](DurableOutputPending6 /\ ~DurableOutputStage7
             => ENABLED <<RetainExactHandoffReceipt>>_typedRolloverVars)
    BY <1>2, PTL
  <1>7. ResponsiveDurableExactOutputSpec =>
          [](DurableOutputPending6 /\ ~DurableOutputStage7
             /\ <<RetainExactHandoffReceipt>>_typedRolloverVars
             => DurableOutputStage7')
    BY <1>3, PTL
  <1>8. ResponsiveDurableExactOutputSpec =>
          [](DurableOutputPending6 /\ ~DurableOutputStage7
             /\ [ResponsiveDurableExactOutputNext]_typedRolloverVars
             => \/ DurableOutputStage7'
                \/ DurableOutputPending6')
    BY <1>1, PTL
  <1> QED BY <1>4, <1>5, <1>6, <1>7, <1>8, PTL
       DEF DurableOutputPending6

THEOREM DurableOutputPending7IsNotOrphaned ==
  /\ DurableOutputPending7
  /\ [ResponsiveDurableExactOutputNext]_typedRolloverVars
  => \/ DurableOutputPending7'
     \/ DurableOutputStage8'
BY ResponsiveDurableOutputStepPreservesCorridorBase, IsaT(600)
   DEF DurableOutputPending7, DurableOutputStage7,
       DurableOutputStage8, DurableOutputCorridorBase,
       DurableOutputGoal, ExactRetainedMergeSidecars,
       ResponsiveDurableExactOutputNext,
       CreateServiceTransportOwnerPair, ValidateFinality,
       CloseWorkerIngress, ClearOneWorkerExactOutput,
       BuildImmediateSuccessor, SealAppliedHeightOutputHandoff,
       RetainExactHandoffReceipt,
       PublishDurableExactOutputSuccessorLifecycleStateSlotV3,
       PublishSuccessorLifecycleStateSlotV3WithAuthority,
       SyncSuccessorLifecycleStateDirectoryV3,
       ReplaceSuccessorLifecycleRootV3,
       CommitSuccessorLifecycleRootV3,
       PublishCommittedLifecycleV3ToMemory

THEOREM DurableOutputPending7EnablesStateSlotPublish ==
  DurableOutputPending7 =>
    ENABLED
      <<PublishDurableExactOutputSuccessorLifecycleStateSlotV3>>_
        typedRolloverVars
BY ExpandENABLED, IsaT(600)
   DEF DurableOutputPending7, DurableOutputStage7,
       DurableOutputStage8, DurableOutputCorridorBase,
       DurableOutputGoal, ExactRetainedMergeSidecars,
       ChangedRosterAuthorityAvailable,
       PublishDurableExactOutputSuccessorLifecycleStateSlotV3,
       PublishSuccessorLifecycleStateSlotV3WithAuthority,
       LifecycleJournalReady, LifecycleMemoryMatchesDurableSnapshotV3,
       typedRolloverVars

THEOREM DurableOutputStateSlotPublishExitsPending7 ==
  /\ DurableOutputPending7
  /\ <<PublishDurableExactOutputSuccessorLifecycleStateSlotV3>>_
       typedRolloverVars
  => DurableOutputStage8'
BY ResponsiveDurableOutputStepPreservesCorridorBase, IsaT(600)
   DEF DurableOutputPending7, DurableOutputStage7,
       DurableOutputStage8, DurableOutputGoal,
       PublishDurableExactOutputSuccessorLifecycleStateSlotV3,
       PublishSuccessorLifecycleStateSlotV3WithAuthority,
       LifecycleSnapshotV3, typedRolloverVars,
       ResponsiveDurableExactOutputNext

THEOREM DurableOutputStage7LeadsToStage8 ==
  ResponsiveDurableExactOutputSpec =>
    (DurableOutputStage7 ~> DurableOutputStage8)
PROOF
  <1>1. /\ DurableOutputPending7
         /\ [ResponsiveDurableExactOutputNext]_typedRolloverVars
         => \/ DurableOutputPending7'
            \/ DurableOutputStage8'
    BY DurableOutputPending7IsNotOrphaned
  <1>2. DurableOutputPending7 =>
          ENABLED
            <<PublishDurableExactOutputSuccessorLifecycleStateSlotV3>>_
              typedRolloverVars
    BY DurableOutputPending7EnablesStateSlotPublish
  <1>3. /\ DurableOutputPending7
         /\ <<PublishDurableExactOutputSuccessorLifecycleStateSlotV3>>_
              typedRolloverVars
         => DurableOutputStage8'
    BY DurableOutputStateSlotPublishExitsPending7
  <1>4. ResponsiveDurableExactOutputSpec =>
          WF_typedRolloverVars(
            PublishDurableExactOutputSuccessorLifecycleStateSlotV3)
    BY DEF ResponsiveDurableExactOutputSpec
  <1>5. ResponsiveDurableExactOutputSpec =>
          [][ResponsiveDurableExactOutputNext]_typedRolloverVars
    BY DEF ResponsiveDurableExactOutputSpec
  <1>6. ResponsiveDurableExactOutputSpec =>
          [](DurableOutputPending7 /\ ~DurableOutputStage8
             => ENABLED
                  <<PublishDurableExactOutputSuccessorLifecycleStateSlotV3>>_
                    typedRolloverVars)
    BY <1>2, PTL
  <1>7. ResponsiveDurableExactOutputSpec =>
          [](DurableOutputPending7 /\ ~DurableOutputStage8
             /\ <<PublishDurableExactOutputSuccessorLifecycleStateSlotV3>>_
                   typedRolloverVars
             => DurableOutputStage8')
    BY <1>3, PTL
  <1>8. ResponsiveDurableExactOutputSpec =>
          [](DurableOutputPending7 /\ ~DurableOutputStage8
             /\ [ResponsiveDurableExactOutputNext]_typedRolloverVars
             => \/ DurableOutputStage8'
                \/ DurableOutputPending7')
    BY <1>1, PTL
  <1> QED BY <1>4, <1>5, <1>6, <1>7, <1>8, PTL
       DEF DurableOutputPending7

THEOREM DurableOutputPending8IsNotOrphaned ==
  /\ DurableOutputPending8
  /\ [ResponsiveDurableExactOutputNext]_typedRolloverVars
  => \/ DurableOutputPending8'
     \/ DurableOutputStage9'
BY ResponsiveDurableOutputStepPreservesCorridorBase, IsaT(600)
   DEF DurableOutputPending8, DurableOutputStage8,
       DurableOutputStage9, DurableOutputCorridorBase,
       DurableOutputGoal, ResponsiveDurableExactOutputNext,
       CreateServiceTransportOwnerPair, ValidateFinality,
       CloseWorkerIngress, ClearOneWorkerExactOutput,
       BuildImmediateSuccessor, SealAppliedHeightOutputHandoff,
       RetainExactHandoffReceipt,
       PublishDurableExactOutputSuccessorLifecycleStateSlotV3,
       PublishSuccessorLifecycleStateSlotV3WithAuthority,
       SyncSuccessorLifecycleStateDirectoryV3,
       ReplaceSuccessorLifecycleRootV3,
       CommitSuccessorLifecycleRootV3,
       PublishCommittedLifecycleV3ToMemory

THEOREM DurableOutputPending8EnablesStateDirectorySync ==
  DurableOutputPending8 =>
    ENABLED <<SyncSuccessorLifecycleStateDirectoryV3>>_typedRolloverVars
BY ExpandENABLED, IsaT(600)
   DEF DurableOutputPending8, DurableOutputStage8,
       DurableOutputStage9, DurableOutputCorridorBase,
       DurableOutputGoal, TypedRolloverSafetyInvariant,
       LifecycleCommitPhaseInvariant,
       ValidatedCandidateSuccessorStateSlotAheadOfRoot,
       DurableCandidateStateSlotAheadOfRoot,
       LifecycleStateDirectoryIsSynced,
       SyncSuccessorLifecycleStateDirectoryV3, typedRolloverVars

THEOREM DurableOutputStateDirectorySyncExitsPending8 ==
  /\ DurableOutputPending8
  /\ <<SyncSuccessorLifecycleStateDirectoryV3>>_typedRolloverVars
  => DurableOutputStage9'
BY ResponsiveDurableOutputStepPreservesCorridorBase, IsaT(600)
   DEF DurableOutputPending8, DurableOutputStage8,
       DurableOutputStage9, DurableOutputGoal,
       SyncSuccessorLifecycleStateDirectoryV3,
       typedRolloverVars, ResponsiveDurableExactOutputNext

THEOREM DurableOutputStage8LeadsToStage9 ==
  ResponsiveDurableExactOutputSpec =>
    (DurableOutputStage8 ~> DurableOutputStage9)
PROOF
  <1>1. /\ DurableOutputPending8
         /\ [ResponsiveDurableExactOutputNext]_typedRolloverVars
         => \/ DurableOutputPending8'
            \/ DurableOutputStage9'
    BY DurableOutputPending8IsNotOrphaned
  <1>2. DurableOutputPending8 =>
          ENABLED
            <<SyncSuccessorLifecycleStateDirectoryV3>>_typedRolloverVars
    BY DurableOutputPending8EnablesStateDirectorySync
  <1>3. /\ DurableOutputPending8
         /\ <<SyncSuccessorLifecycleStateDirectoryV3>>_typedRolloverVars
         => DurableOutputStage9'
    BY DurableOutputStateDirectorySyncExitsPending8
  <1>4. ResponsiveDurableExactOutputSpec =>
          WF_typedRolloverVars(SyncSuccessorLifecycleStateDirectoryV3)
    BY DEF ResponsiveDurableExactOutputSpec
  <1>5. ResponsiveDurableExactOutputSpec =>
          [][ResponsiveDurableExactOutputNext]_typedRolloverVars
    BY DEF ResponsiveDurableExactOutputSpec
  <1>6. ResponsiveDurableExactOutputSpec =>
          [](DurableOutputPending8 /\ ~DurableOutputStage9
             => ENABLED
                  <<SyncSuccessorLifecycleStateDirectoryV3>>_
                    typedRolloverVars)
    BY <1>2, PTL
  <1>7. ResponsiveDurableExactOutputSpec =>
          [](DurableOutputPending8 /\ ~DurableOutputStage9
             /\ <<SyncSuccessorLifecycleStateDirectoryV3>>_
                   typedRolloverVars
             => DurableOutputStage9')
    BY <1>3, PTL
  <1>8. ResponsiveDurableExactOutputSpec =>
          [](DurableOutputPending8 /\ ~DurableOutputStage9
             /\ [ResponsiveDurableExactOutputNext]_typedRolloverVars
             => \/ DurableOutputStage9'
                \/ DurableOutputPending8')
    BY <1>1, PTL
  <1> QED BY <1>4, <1>5, <1>6, <1>7, <1>8, PTL
       DEF DurableOutputPending8

THEOREM DurableOutputPending9IsNotOrphaned ==
  /\ DurableOutputPending9
  /\ [ResponsiveDurableExactOutputNext]_typedRolloverVars
  => \/ DurableOutputPending9'
     \/ DurableOutputStage10'
BY ResponsiveDurableOutputStepPreservesCorridorBase, IsaT(600)
   DEF DurableOutputPending9, DurableOutputStage9,
       DurableOutputStage10, DurableOutputCorridorBase,
       DurableOutputGoal, ResponsiveDurableExactOutputNext,
       CreateServiceTransportOwnerPair, ValidateFinality,
       CloseWorkerIngress, ClearOneWorkerExactOutput,
       BuildImmediateSuccessor, SealAppliedHeightOutputHandoff,
       RetainExactHandoffReceipt,
       PublishDurableExactOutputSuccessorLifecycleStateSlotV3,
       PublishSuccessorLifecycleStateSlotV3WithAuthority,
       SyncSuccessorLifecycleStateDirectoryV3,
       ReplaceSuccessorLifecycleRootV3,
       CommitSuccessorLifecycleRootV3,
       PublishCommittedLifecycleV3ToMemory

THEOREM DurableOutputPending9EnablesRootReplacement ==
  DurableOutputPending9 =>
    ENABLED <<ReplaceSuccessorLifecycleRootV3>>_typedRolloverVars
BY ExpandENABLED, IsaT(600)
   DEF DurableOutputPending9, DurableOutputStage9,
       DurableOutputStage10, DurableOutputCorridorBase,
       DurableOutputGoal, TypedRolloverSafetyInvariant,
       LifecycleCommitPhaseInvariant,
       DurableCandidateStateSlotAheadOfRoot,
       LifecycleStateDirectoryIsSynced,
       LifecycleRootDirectoryIsSynced,
       ReplaceSuccessorLifecycleRootV3, typedRolloverVars

THEOREM DurableOutputRootReplacementExitsPending9 ==
  /\ DurableOutputPending9
  /\ <<ReplaceSuccessorLifecycleRootV3>>_typedRolloverVars
  => DurableOutputStage10'
BY ResponsiveDurableOutputStepPreservesCorridorBase, IsaT(600)
   DEF DurableOutputPending9, DurableOutputStage9,
       DurableOutputStage10, DurableOutputGoal,
       ReplaceSuccessorLifecycleRootV3,
       typedRolloverVars, ResponsiveDurableExactOutputNext

THEOREM DurableOutputStage9LeadsToStage10 ==
  ResponsiveDurableExactOutputSpec =>
    (DurableOutputStage9 ~> DurableOutputStage10)
PROOF
  <1>1. /\ DurableOutputPending9
         /\ [ResponsiveDurableExactOutputNext]_typedRolloverVars
         => \/ DurableOutputPending9'
            \/ DurableOutputStage10'
    BY DurableOutputPending9IsNotOrphaned
  <1>2. DurableOutputPending9 =>
          ENABLED <<ReplaceSuccessorLifecycleRootV3>>_typedRolloverVars
    BY DurableOutputPending9EnablesRootReplacement
  <1>3. /\ DurableOutputPending9
         /\ <<ReplaceSuccessorLifecycleRootV3>>_typedRolloverVars
         => DurableOutputStage10'
    BY DurableOutputRootReplacementExitsPending9
  <1>4. ResponsiveDurableExactOutputSpec =>
          WF_typedRolloverVars(ReplaceSuccessorLifecycleRootV3)
    BY DEF ResponsiveDurableExactOutputSpec
  <1>5. ResponsiveDurableExactOutputSpec =>
          [][ResponsiveDurableExactOutputNext]_typedRolloverVars
    BY DEF ResponsiveDurableExactOutputSpec
  <1>6. ResponsiveDurableExactOutputSpec =>
          [](DurableOutputPending9 /\ ~DurableOutputStage10
             => ENABLED
                  <<ReplaceSuccessorLifecycleRootV3>>_typedRolloverVars)
    BY <1>2, PTL
  <1>7. ResponsiveDurableExactOutputSpec =>
          [](DurableOutputPending9 /\ ~DurableOutputStage10
             /\ <<ReplaceSuccessorLifecycleRootV3>>_typedRolloverVars
             => DurableOutputStage10')
    BY <1>3, PTL
  <1>8. ResponsiveDurableExactOutputSpec =>
          [](DurableOutputPending9 /\ ~DurableOutputStage10
             /\ [ResponsiveDurableExactOutputNext]_typedRolloverVars
             => \/ DurableOutputStage10'
                \/ DurableOutputPending9')
    BY <1>1, PTL
  <1> QED BY <1>4, <1>5, <1>6, <1>7, <1>8, PTL
       DEF DurableOutputPending9

THEOREM DurableOutputPending10IsNotOrphaned ==
  /\ DurableOutputPending10
  /\ [ResponsiveDurableExactOutputNext]_typedRolloverVars
  => \/ DurableOutputPending10'
     \/ DurableOutputStage11'
BY ResponsiveDurableOutputStepPreservesCorridorBase, IsaT(600)
   DEF DurableOutputPending10, DurableOutputStage10,
       DurableOutputStage11, DurableOutputCorridorBase,
       DurableOutputGoal, ResponsiveDurableExactOutputNext,
       CreateServiceTransportOwnerPair, ValidateFinality,
       CloseWorkerIngress, ClearOneWorkerExactOutput,
       BuildImmediateSuccessor, SealAppliedHeightOutputHandoff,
       RetainExactHandoffReceipt,
       PublishDurableExactOutputSuccessorLifecycleStateSlotV3,
       PublishSuccessorLifecycleStateSlotV3WithAuthority,
       SyncSuccessorLifecycleStateDirectoryV3,
       ReplaceSuccessorLifecycleRootV3,
       CommitSuccessorLifecycleRootV3,
       PublishCommittedLifecycleV3ToMemory

THEOREM DurableOutputPending10EnablesRootCommit ==
  DurableOutputPending10 =>
    ENABLED <<CommitSuccessorLifecycleRootV3>>_typedRolloverVars
BY ExpandENABLED, IsaT(600)
   DEF DurableOutputPending10, DurableOutputStage10,
       DurableOutputStage11, DurableOutputCorridorBase,
       DurableOutputGoal, TypedRolloverSafetyInvariant,
       LifecycleCommitPhaseInvariant,
       RootSelectedSuccessorAheadOfMemory,
       RootSelectedLifecyclePairMatches,
       LifecycleSnapshotSemanticallyValid,
       LifecycleStateDirectoryIsSynced,
       LifecycleRootDirectoryIsSynced,
       CommitSuccessorLifecycleRootV3, typedRolloverVars

THEOREM DurableOutputRootCommitExitsPending10 ==
  /\ DurableOutputPending10
  /\ <<CommitSuccessorLifecycleRootV3>>_typedRolloverVars
  => DurableOutputStage11'
BY ResponsiveDurableOutputStepPreservesCorridorBase, IsaT(600)
   DEF DurableOutputPending10, DurableOutputStage10,
       DurableOutputStage11, DurableOutputGoal,
       CommitSuccessorLifecycleRootV3,
       typedRolloverVars, ResponsiveDurableExactOutputNext

THEOREM DurableOutputStage10LeadsToStage11 ==
  ResponsiveDurableExactOutputSpec =>
    (DurableOutputStage10 ~> DurableOutputStage11)
PROOF
  <1>1. /\ DurableOutputPending10
         /\ [ResponsiveDurableExactOutputNext]_typedRolloverVars
         => \/ DurableOutputPending10'
            \/ DurableOutputStage11'
    BY DurableOutputPending10IsNotOrphaned
  <1>2. DurableOutputPending10 =>
          ENABLED <<CommitSuccessorLifecycleRootV3>>_typedRolloverVars
    BY DurableOutputPending10EnablesRootCommit
  <1>3. /\ DurableOutputPending10
         /\ <<CommitSuccessorLifecycleRootV3>>_typedRolloverVars
         => DurableOutputStage11'
    BY DurableOutputRootCommitExitsPending10
  <1>4. ResponsiveDurableExactOutputSpec =>
          WF_typedRolloverVars(CommitSuccessorLifecycleRootV3)
    BY DEF ResponsiveDurableExactOutputSpec
  <1>5. ResponsiveDurableExactOutputSpec =>
          [][ResponsiveDurableExactOutputNext]_typedRolloverVars
    BY DEF ResponsiveDurableExactOutputSpec
  <1>6. ResponsiveDurableExactOutputSpec =>
          [](DurableOutputPending10 /\ ~DurableOutputStage11
             => ENABLED
                  <<CommitSuccessorLifecycleRootV3>>_typedRolloverVars)
    BY <1>2, PTL
  <1>7. ResponsiveDurableExactOutputSpec =>
          [](DurableOutputPending10 /\ ~DurableOutputStage11
             /\ <<CommitSuccessorLifecycleRootV3>>_typedRolloverVars
             => DurableOutputStage11')
    BY <1>3, PTL
  <1>8. ResponsiveDurableExactOutputSpec =>
          [](DurableOutputPending10 /\ ~DurableOutputStage11
             /\ [ResponsiveDurableExactOutputNext]_typedRolloverVars
             => \/ DurableOutputStage11'
                \/ DurableOutputPending10')
    BY <1>1, PTL
  <1> QED BY <1>4, <1>5, <1>6, <1>7, <1>8, PTL
       DEF DurableOutputPending10

THEOREM DurableOutputPending11IsNotOrphaned ==
  /\ DurableOutputPending11
  /\ [ResponsiveDurableExactOutputNext]_typedRolloverVars
  => \/ DurableOutputPending11'
     \/ DurableOutputGoal'
BY ResponsiveDurableOutputStepPreservesCorridorBase, IsaT(600)
   DEF DurableOutputPending11, DurableOutputStage11,
       DurableOutputCorridorBase, DurableOutputGoal,
       DurableExactOutputSuccessorActiveWithoutRestart,
       ResponsiveDurableExactOutputNext,
       CreateServiceTransportOwnerPair, ValidateFinality,
       CloseWorkerIngress, ClearOneWorkerExactOutput,
       BuildImmediateSuccessor, SealAppliedHeightOutputHandoff,
       RetainExactHandoffReceipt,
       PublishDurableExactOutputSuccessorLifecycleStateSlotV3,
       PublishSuccessorLifecycleStateSlotV3WithAuthority,
       SyncSuccessorLifecycleStateDirectoryV3,
       ReplaceSuccessorLifecycleRootV3,
       CommitSuccessorLifecycleRootV3,
       PublishCommittedLifecycleV3ToMemory

THEOREM DurableOutputPending11EnablesMemoryPublication ==
  DurableOutputPending11 =>
    ENABLED <<PublishCommittedLifecycleV3ToMemory>>_typedRolloverVars
BY ExpandENABLED, IsaT(600)
   DEF DurableOutputPending11, DurableOutputStage11,
       DurableOutputCorridorBase, DurableOutputGoal,
       TypedRolloverSafetyInvariant, LifecycleCommitPhaseInvariant,
       RootCommittedSuccessorAheadOfMemory,
       PublishCommittedLifecycleV3ToMemory, typedRolloverVars

THEOREM DurableOutputMemoryPublicationExitsPending11 ==
  /\ DurableOutputPending11
  /\ <<PublishCommittedLifecycleV3ToMemory>>_typedRolloverVars
  => DurableOutputGoal'
BY ResponsiveDurableOutputStepPreservesCorridorBase, IsaT(600)
   DEF DurableOutputPending11, DurableOutputStage11,
       DurableOutputGoal,
       DurableExactOutputSuccessorActiveWithoutRestart,
       PublishCommittedLifecycleV3ToMemory,
       typedRolloverVars, ResponsiveDurableExactOutputNext

THEOREM DurableOutputStage11LeadsToGoal ==
  ResponsiveDurableExactOutputSpec =>
    (DurableOutputStage11 ~> DurableOutputGoal)
PROOF
  <1>1. /\ DurableOutputPending11
         /\ [ResponsiveDurableExactOutputNext]_typedRolloverVars
         => \/ DurableOutputPending11'
            \/ DurableOutputGoal'
    BY DurableOutputPending11IsNotOrphaned
  <1>2. DurableOutputPending11 =>
          ENABLED <<PublishCommittedLifecycleV3ToMemory>>_typedRolloverVars
    BY DurableOutputPending11EnablesMemoryPublication
  <1>3. /\ DurableOutputPending11
         /\ <<PublishCommittedLifecycleV3ToMemory>>_typedRolloverVars
         => DurableOutputGoal'
    BY DurableOutputMemoryPublicationExitsPending11
  <1>4. ResponsiveDurableExactOutputSpec =>
          WF_typedRolloverVars(PublishCommittedLifecycleV3ToMemory)
    BY DEF ResponsiveDurableExactOutputSpec
  <1>5. ResponsiveDurableExactOutputSpec =>
          [][ResponsiveDurableExactOutputNext]_typedRolloverVars
    BY DEF ResponsiveDurableExactOutputSpec
  <1>6. ResponsiveDurableExactOutputSpec =>
          [](DurableOutputPending11 /\ ~DurableOutputGoal
             => ENABLED
                  <<PublishCommittedLifecycleV3ToMemory>>_
                    typedRolloverVars)
    BY <1>2, PTL
  <1>7. ResponsiveDurableExactOutputSpec =>
          [](DurableOutputPending11 /\ ~DurableOutputGoal
             /\ <<PublishCommittedLifecycleV3ToMemory>>_typedRolloverVars
             => DurableOutputGoal')
    BY <1>3, PTL
  <1>8. ResponsiveDurableExactOutputSpec =>
          [](DurableOutputPending11 /\ ~DurableOutputGoal
             /\ [ResponsiveDurableExactOutputNext]_typedRolloverVars
             => \/ DurableOutputGoal'
                \/ DurableOutputPending11')
    BY <1>1, PTL
  <1> QED BY <1>4, <1>5, <1>6, <1>7, <1>8, PTL
       DEF DurableOutputPending11

(***************************************************************************
These are separate temporal debts.  The first is the healthy process-local
durable exact-output corridor.  The second begins with no process-local
receipt and requires validated restart, ordered resynchronization, cleanup,
and the distinct RestartRestore authority.
***************************************************************************)
THEOREM ResponsiveDurableExactOutputRolloverLivenessObligation ==
  ResponsiveDurableExactOutputSpec =>
    ResponsiveDurableExactOutputRolloverLiveness
PROOF
  <1>1. ASSUME ResponsiveDurableExactOutputSpec
         PROVE state.finalityValidated ~> DurableOutputGoal
    <2>1. []DurableOutputCorridorBase
      BY <1>1, ResponsiveDurableOutputAlwaysCorridorBase
    <2>2. state.finalityValidated ~> DurableOutputStage0
      BY <2>1, PTL DEF DurableOutputStage0
    <2>3. DurableOutputStage0 ~> DurableOutputStage1
      BY <1>1, DurableOutputStage0LeadsToStage1
    <2>4. DurableOutputStage1 ~> DurableOutputStage2
      BY <1>1, DurableOutputStage1LeadsToStage2
    <2>5. DurableOutputStage2 ~> DurableOutputStage3
      BY <1>1, DurableOutputStage2LeadsToStage3
    <2>6. DurableOutputStage3 ~> DurableOutputStage4
      BY <1>1, DurableOutputStage3LeadsToStage4
    <2>7. DurableOutputStage4 ~> DurableOutputStage5
      BY <1>1, DurableOutputStage4LeadsToStage5
    <2>8. DurableOutputStage5 ~> DurableOutputStage6
      BY <1>1, DurableOutputStage5LeadsToStage6
    <2>9. DurableOutputStage6 ~> DurableOutputStage7
      BY <1>1, DurableOutputStage6LeadsToStage7
    <2>10. DurableOutputStage7 ~> DurableOutputStage8
      BY <1>1, DurableOutputStage7LeadsToStage8
    <2>11. DurableOutputStage8 ~> DurableOutputStage9
      BY <1>1, DurableOutputStage8LeadsToStage9
    <2>12. DurableOutputStage9 ~> DurableOutputStage10
      BY <1>1, DurableOutputStage9LeadsToStage10
    <2>13. DurableOutputStage10 ~> DurableOutputStage11
      BY <1>1, DurableOutputStage10LeadsToStage11
    <2>14. DurableOutputStage11 ~> DurableOutputGoal
      BY <1>1, DurableOutputStage11LeadsToGoal
    <2> QED BY <2>2, <2>3, <2>4, <2>5, <2>6, <2>7,
                 <2>8, <2>9, <2>10, <2>11, <2>12, <2>13,
                 <2>14, PTL
  <1> QED BY <1>1
       DEF ResponsiveDurableExactOutputRolloverLiveness,
           DurableOutputGoal

(***************************************************************************
Responsive validated-restart corridor.

This rank starts with the committed root-selected predecessor and no
process-local receipt.  Validation, the two ordered parent-directory syncs,
cleanup, and predecessor recovery are separate fair stages.  Only the
recovered RestartRestore fence may then enter the same four-step V3 commit
corridor used above.
***************************************************************************)
LOCAL RestartRestoreGoal ==
  RestartRestoreSuccessorActiveWithoutRestart

LOCAL RestartRestoreCorridorBase ==
  /\ TypedRolloverSafetyInvariant
  /\ state.startupMode = "UnvalidatedRestart"
  /\ state.startupValidationFault = "None"
  /\ state.requesterEpochPhase = "Idle"
  /\ state.targetRoster # state.baselineRoster
  /\ state.serviceGeneration < ServiceGenerationLimit
  /\ state.receiptStage \in {"Absent", "Lost"}
  /\ \/ RestartRestoreGoal
     \/ /\ state.currentRoster = state.baselineRoster
        /\ ~state.successorActive
        /\ (state.restartRequired =>
              /\ state.lifecycleCommitPhase = "Restarting"
              /\ state.transitionAuthority = "None"
              /\ (state.durableJournalValidated <=>
                    state.validatedRestartObserved)
              /\ LifecycleMemoryMatchesDurableSnapshotV3
              /\ (state.crashArtifactsPresent <=>
                    ~state.cleanupPerformed))
        /\ (~state.restartRequired =>
              /\ state.transitionAuthority = "None"
              /\ ChangedRosterReplacementNeeded)
        /\ (state.lifecycleCommitPhase = "Current" =>
              /\ state.durableLifecycleRootV3.rootGeneration <
                   RootGenerationLimit
              /\ LifecycleJournalReady(state)
              /\ LifecycleMemoryMatchesDurableSnapshotV3)

LOCAL RestartRestoreStage0 ==
  \/ RestartRestoreGoal
  \/ /\ RestartRestoreCorridorBase
     /\ state.restartRequired

LOCAL RestartRestoreStage1 ==
  \/ RestartRestoreGoal
  \/ /\ RestartRestoreCorridorBase
     /\ state.restartRequired
     /\ state.durableJournalValidated
     /\ state.validatedRestartObserved

LOCAL RestartRestoreStage2 ==
  \/ RestartRestoreGoal
  \/ /\ RestartRestoreCorridorBase
     /\ state.restartRequired
     /\ state.durableJournalValidated
     /\ state.validatedRestartObserved
     /\ state.restartStateDirectoryResynced

LOCAL RestartRestoreStage3 ==
  \/ RestartRestoreGoal
  \/ /\ RestartRestoreCorridorBase
     /\ state.restartRequired
     /\ state.durableJournalValidated
     /\ state.validatedRestartObserved
     /\ state.restartStateDirectoryResynced
     /\ state.restartRootDirectoryResynced

LOCAL RestartRestoreStage4 ==
  \/ RestartRestoreGoal
  \/ /\ RestartRestoreCorridorBase
     /\ state.restartRequired
     /\ state.durableJournalValidated
     /\ state.validatedRestartObserved
     /\ state.restartStateDirectoryResynced
     /\ state.restartRootDirectoryResynced
     /\ state.cleanupPerformed
     /\ ~state.crashArtifactsPresent

LOCAL RestartRestoreStage5 ==
  \/ RestartRestoreGoal
  \/ /\ RestartRestoreCorridorBase
     /\ ~state.restartRequired
     /\ state.lifecycleCommitPhase = "Current"
     /\ state.validatedRestartObserved
     /\ state.restartFenceAuthorized

LOCAL RestartRestoreStage6 ==
  \/ RestartRestoreGoal
  \/ /\ RestartRestoreCorridorBase
     /\ state.lifecycleCommitPhase = "StateSlotReplaced"
     /\ state.pendingRolloverAuthority = "RestartRestore"

LOCAL RestartRestoreStage7 ==
  \/ RestartRestoreGoal
  \/ /\ RestartRestoreCorridorBase
     /\ state.lifecycleCommitPhase = "StateSlotPublished"
     /\ state.pendingRolloverAuthority = "RestartRestore"

LOCAL RestartRestoreStage8 ==
  \/ RestartRestoreGoal
  \/ /\ RestartRestoreCorridorBase
     /\ state.lifecycleCommitPhase = "RootReplaced"
     /\ state.pendingRolloverAuthority = "RestartRestore"

LOCAL RestartRestoreStage9 ==
  \/ RestartRestoreGoal
  \/ /\ RestartRestoreCorridorBase
     /\ state.lifecycleCommitPhase = "RootCommitted"
     /\ state.pendingRolloverAuthority = "RestartRestore"

LOCAL RestartRestorePending0 ==
  RestartRestoreStage0 /\ ~RestartRestoreStage1
LOCAL RestartRestorePending1 ==
  RestartRestoreStage1 /\ ~RestartRestoreStage2
LOCAL RestartRestorePending2 ==
  RestartRestoreStage2 /\ ~RestartRestoreStage3
LOCAL RestartRestorePending3 ==
  RestartRestoreStage3 /\ ~RestartRestoreStage4
LOCAL RestartRestorePending4 ==
  RestartRestoreStage4 /\ ~RestartRestoreStage5
LOCAL RestartRestorePending5 ==
  RestartRestoreStage5 /\ ~RestartRestoreStage6
LOCAL RestartRestorePending6 ==
  RestartRestoreStage6 /\ ~RestartRestoreStage7
LOCAL RestartRestorePending7 ==
  RestartRestoreStage7 /\ ~RestartRestoreStage8
LOCAL RestartRestorePending8 ==
  RestartRestoreStage8 /\ ~RestartRestoreStage9
LOCAL RestartRestorePending9 ==
  RestartRestoreStage9 /\ ~RestartRestoreGoal

THEOREM ResponsiveRestartRestoreInitEstablishesCorridorBase ==
  ResponsiveRestartRestoreInit => RestartRestoreCorridorBase
BY TypedRolloverInitEstablishesSafetyObligation, IsaT(600)
   DEF ResponsiveRestartRestoreInit, RestartRestoreCorridorBase,
       RestartRestoreGoal,
       RestartRestoreSuccessorActiveWithoutRestart,
       ChangedRosterReplacementNeeded, CompactionCauseMatchesGeometry,
       LifecycleJournalReady, LifecycleMemoryMatchesDurableSnapshotV3,
       PersistentLifecycleMemoryMatchesSnapshot,
       RootSelectedLifecyclePairMatches,
       RootSelectedLifecyclePairIsPresent,
       LifecycleSnapshotSemanticallyValid,
       LifecycleStateDirectoryIsSynced,
       LifecycleRootDirectoryIsSynced, DurableSnapshot,
       SelectedLifecycleSnapshotV3, SelectedLifecycleStateSlot,
       Init, InitialLifecycleSnapshotV3, LiveLifecycleSnapshotV3,
       InitialLifecycleStateSlotsV3, LifecycleSnapshotV3,
       LifecycleRootV3, LifecycleSnapshotDigest,
       LifecycleStateSlot, NoLifecycleSnapshot

THEOREM ResponsiveRestartRestoreStepPreservesCorridorBase ==
  /\ RestartRestoreCorridorBase
  /\ [ResponsiveRestartRestoreNext]_typedRolloverVars
  => RestartRestoreCorridorBase'
PROOF
  <1>1. CASE ResponsiveRestartRestoreNext
    <2>1. TypedRolloverSafetyInvariant'
      BY <1>1, TypedRolloverNextPreservesSafetyObligation
         DEF ResponsiveRestartRestoreNext, Next
    <2>2. RestartRestoreCorridorBase'
      BY <1>1, <2>1, IsaT(900)
         DEF RestartRestoreCorridorBase, RestartRestoreGoal,
             RestartRestoreSuccessorActiveWithoutRestart,
             ChangedRosterReplacementNeeded,
             CompactionCauseMatchesGeometry, LifecycleJournalReady,
             LifecycleMemoryMatchesDurableSnapshotV3,
             PersistentLifecycleMemoryMatchesSnapshot,
             RootSelectedLifecyclePairMatches,
             RootSelectedLifecyclePairIsPresent,
             LifecycleSnapshotSemanticallyValid,
             LifecycleStateDirectoryIsSynced,
             LifecycleRootDirectoryIsSynced, DurableSnapshot,
             SelectedLifecycleSnapshotV3,
             SelectedLifecycleStateSlot,
             ResponsiveRestartRestoreNext,
             ValidateRootSelectedLifecycleV3,
             ResyncValidatedLifecycleStateDirectoryV3,
             ResyncValidatedLifecycleRootDirectoryV3,
             CleanupValidatedLifecycleArtifactsV3,
             RecoverPredecessorLifecycleV3,
             PublishRestartRestoreSuccessorLifecycleStateSlotV3,
             PublishSuccessorLifecycleStateSlotV3WithAuthority,
             SyncSuccessorLifecycleStateDirectoryV3,
             ReplaceSuccessorLifecycleRootV3,
             CommitSuccessorLifecycleRootV3,
             PublishCommittedLifecycleV3ToMemory,
             LifecycleRootV3, LifecycleSnapshotV3,
             LifecycleSnapshotDigest, LifecycleStateSlot
    <2> QED BY <2>2
  <1>2. CASE UNCHANGED typedRolloverVars
    BY <1>2, IsaT(120)
       DEF typedRolloverVars, RestartRestoreCorridorBase
  <1> QED BY <1>1, <1>2

THEOREM ResponsiveRestartRestoreAlwaysCorridorBase ==
  ResponsiveRestartRestoreSpec => []RestartRestoreCorridorBase
PROOF
  <1>1. ResponsiveRestartRestoreInit => RestartRestoreCorridorBase
    BY ResponsiveRestartRestoreInitEstablishesCorridorBase
  <1>2. /\ RestartRestoreCorridorBase
         /\ [ResponsiveRestartRestoreNext]_typedRolloverVars
         => RestartRestoreCorridorBase'
    BY ResponsiveRestartRestoreStepPreservesCorridorBase
  <1>3. RestartRestoreCorridorBase
         /\ [][ResponsiveRestartRestoreNext]_typedRolloverVars
         => []RestartRestoreCorridorBase
    BY <1>2
  <1> QED BY <1>1, <1>3, PTL
       DEF ResponsiveRestartRestoreSpec

THEOREM RestartRestorePending0IsNotOrphaned ==
  /\ RestartRestorePending0
  /\ [ResponsiveRestartRestoreNext]_typedRolloverVars
  => \/ RestartRestorePending0'
     \/ RestartRestoreStage1'
BY ResponsiveRestartRestoreStepPreservesCorridorBase, IsaT(600)
   DEF RestartRestorePending0, RestartRestoreStage0,
       RestartRestoreStage1, RestartRestoreCorridorBase,
       RestartRestoreGoal, ResponsiveRestartRestoreNext,
       ValidateRootSelectedLifecycleV3,
       ResyncValidatedLifecycleStateDirectoryV3,
       ResyncValidatedLifecycleRootDirectoryV3,
       CleanupValidatedLifecycleArtifactsV3,
       RecoverPredecessorLifecycleV3,
       PublishRestartRestoreSuccessorLifecycleStateSlotV3,
       PublishSuccessorLifecycleStateSlotV3WithAuthority,
       SyncSuccessorLifecycleStateDirectoryV3,
       ReplaceSuccessorLifecycleRootV3,
       CommitSuccessorLifecycleRootV3,
       PublishCommittedLifecycleV3ToMemory

THEOREM RestartRestorePending0EnablesValidation ==
  RestartRestorePending0 =>
    ENABLED <<ValidateRootSelectedLifecycleV3>>_typedRolloverVars
BY ExpandENABLED, IsaT(600)
   DEF RestartRestorePending0, RestartRestoreStage0,
       RestartRestoreStage1, RestartRestoreCorridorBase,
       RestartRestoreGoal, TypedRolloverSafetyInvariant,
       RootAnchoredLifecycleV3Invariant,
       RootSelectedLifecyclePairMatches,
       LifecycleSnapshotSemanticallyValid, DurableSnapshot,
       LifecycleRootShapeIsValid,
       ValidateRootSelectedLifecycleV3, typedRolloverVars

THEOREM RestartRestoreValidationExitsPending0 ==
  /\ RestartRestorePending0
  /\ <<ValidateRootSelectedLifecycleV3>>_typedRolloverVars
  => RestartRestoreStage1'
BY ResponsiveRestartRestoreStepPreservesCorridorBase, IsaT(300)
   DEF RestartRestorePending0, RestartRestoreStage0,
       RestartRestoreStage1, RestartRestoreGoal,
       ValidateRootSelectedLifecycleV3, typedRolloverVars,
       ResponsiveRestartRestoreNext

THEOREM RestartRestoreStage0LeadsToStage1 ==
  ResponsiveRestartRestoreSpec =>
    (RestartRestoreStage0 ~> RestartRestoreStage1)
PROOF
  <1>1. /\ RestartRestorePending0
         /\ [ResponsiveRestartRestoreNext]_typedRolloverVars
         => \/ RestartRestorePending0'
            \/ RestartRestoreStage1'
    BY RestartRestorePending0IsNotOrphaned
  <1>2. RestartRestorePending0 =>
          ENABLED <<ValidateRootSelectedLifecycleV3>>_typedRolloverVars
    BY RestartRestorePending0EnablesValidation
  <1>3. /\ RestartRestorePending0
         /\ <<ValidateRootSelectedLifecycleV3>>_typedRolloverVars
         => RestartRestoreStage1'
    BY RestartRestoreValidationExitsPending0
  <1>4. ResponsiveRestartRestoreSpec =>
          WF_typedRolloverVars(ValidateRootSelectedLifecycleV3)
    BY DEF ResponsiveRestartRestoreSpec
  <1>5. ResponsiveRestartRestoreSpec =>
          [][ResponsiveRestartRestoreNext]_typedRolloverVars
    BY DEF ResponsiveRestartRestoreSpec
  <1>6. ResponsiveRestartRestoreSpec =>
          [](RestartRestorePending0 /\ ~RestartRestoreStage1
             => ENABLED
                  <<ValidateRootSelectedLifecycleV3>>_typedRolloverVars)
    BY <1>2, PTL
  <1>7. ResponsiveRestartRestoreSpec =>
          [](RestartRestorePending0 /\ ~RestartRestoreStage1
             /\ <<ValidateRootSelectedLifecycleV3>>_typedRolloverVars
             => RestartRestoreStage1')
    BY <1>3, PTL
  <1>8. ResponsiveRestartRestoreSpec =>
          [](RestartRestorePending0 /\ ~RestartRestoreStage1
             /\ [ResponsiveRestartRestoreNext]_typedRolloverVars
             => \/ RestartRestoreStage1'
                \/ RestartRestorePending0')
    BY <1>1, PTL
  <1> QED BY <1>4, <1>5, <1>6, <1>7, <1>8, PTL
       DEF RestartRestorePending0

THEOREM RestartRestorePending1IsNotOrphaned ==
  /\ RestartRestorePending1
  /\ [ResponsiveRestartRestoreNext]_typedRolloverVars
  => \/ RestartRestorePending1'
     \/ RestartRestoreStage2'
BY ResponsiveRestartRestoreStepPreservesCorridorBase, IsaT(600)
   DEF RestartRestorePending1, RestartRestoreStage1,
       RestartRestoreStage2, RestartRestoreCorridorBase,
       RestartRestoreGoal, ResponsiveRestartRestoreNext,
       ValidateRootSelectedLifecycleV3,
       ResyncValidatedLifecycleStateDirectoryV3,
       ResyncValidatedLifecycleRootDirectoryV3,
       CleanupValidatedLifecycleArtifactsV3,
       RecoverPredecessorLifecycleV3,
       PublishRestartRestoreSuccessorLifecycleStateSlotV3,
       PublishSuccessorLifecycleStateSlotV3WithAuthority,
       SyncSuccessorLifecycleStateDirectoryV3,
       ReplaceSuccessorLifecycleRootV3,
       CommitSuccessorLifecycleRootV3,
       PublishCommittedLifecycleV3ToMemory

THEOREM RestartRestorePending1EnablesStateResync ==
  RestartRestorePending1 =>
    ENABLED
      <<ResyncValidatedLifecycleStateDirectoryV3>>_typedRolloverVars
BY ExpandENABLED, IsaT(300)
   DEF RestartRestorePending1, RestartRestoreStage1,
       RestartRestoreStage2, RestartRestoreCorridorBase,
       RestartRestoreGoal,
       ResyncValidatedLifecycleStateDirectoryV3, typedRolloverVars

THEOREM RestartRestoreStateResyncExitsPending1 ==
  /\ RestartRestorePending1
  /\ <<ResyncValidatedLifecycleStateDirectoryV3>>_typedRolloverVars
  => RestartRestoreStage2'
BY ResponsiveRestartRestoreStepPreservesCorridorBase, IsaT(300)
   DEF RestartRestorePending1, RestartRestoreStage1,
       RestartRestoreStage2, RestartRestoreGoal,
       ResyncValidatedLifecycleStateDirectoryV3,
       typedRolloverVars, ResponsiveRestartRestoreNext

THEOREM RestartRestoreStage1LeadsToStage2 ==
  ResponsiveRestartRestoreSpec =>
    (RestartRestoreStage1 ~> RestartRestoreStage2)
PROOF
  <1>1. /\ RestartRestorePending1
         /\ [ResponsiveRestartRestoreNext]_typedRolloverVars
         => \/ RestartRestorePending1'
            \/ RestartRestoreStage2'
    BY RestartRestorePending1IsNotOrphaned
  <1>2. RestartRestorePending1 =>
          ENABLED
            <<ResyncValidatedLifecycleStateDirectoryV3>>_typedRolloverVars
    BY RestartRestorePending1EnablesStateResync
  <1>3. /\ RestartRestorePending1
         /\ <<ResyncValidatedLifecycleStateDirectoryV3>>_typedRolloverVars
         => RestartRestoreStage2'
    BY RestartRestoreStateResyncExitsPending1
  <1>4. ResponsiveRestartRestoreSpec =>
          WF_typedRolloverVars(
            ResyncValidatedLifecycleStateDirectoryV3)
    BY DEF ResponsiveRestartRestoreSpec
  <1>5. ResponsiveRestartRestoreSpec =>
          [][ResponsiveRestartRestoreNext]_typedRolloverVars
    BY DEF ResponsiveRestartRestoreSpec
  <1>6. ResponsiveRestartRestoreSpec =>
          [](RestartRestorePending1 /\ ~RestartRestoreStage2
             => ENABLED
                  <<ResyncValidatedLifecycleStateDirectoryV3>>_
                    typedRolloverVars)
    BY <1>2, PTL
  <1>7. ResponsiveRestartRestoreSpec =>
          [](RestartRestorePending1 /\ ~RestartRestoreStage2
             /\ <<ResyncValidatedLifecycleStateDirectoryV3>>_
                   typedRolloverVars
             => RestartRestoreStage2')
    BY <1>3, PTL
  <1>8. ResponsiveRestartRestoreSpec =>
          [](RestartRestorePending1 /\ ~RestartRestoreStage2
             /\ [ResponsiveRestartRestoreNext]_typedRolloverVars
             => \/ RestartRestoreStage2'
                \/ RestartRestorePending1')
    BY <1>1, PTL
  <1> QED BY <1>4, <1>5, <1>6, <1>7, <1>8, PTL
       DEF RestartRestorePending1

THEOREM RestartRestorePending2IsNotOrphaned ==
  /\ RestartRestorePending2
  /\ [ResponsiveRestartRestoreNext]_typedRolloverVars
  => \/ RestartRestorePending2'
     \/ RestartRestoreStage3'
BY ResponsiveRestartRestoreStepPreservesCorridorBase, IsaT(600)
   DEF RestartRestorePending2, RestartRestoreStage2,
       RestartRestoreStage3, RestartRestoreCorridorBase,
       RestartRestoreGoal, ResponsiveRestartRestoreNext,
       ValidateRootSelectedLifecycleV3,
       ResyncValidatedLifecycleStateDirectoryV3,
       ResyncValidatedLifecycleRootDirectoryV3,
       CleanupValidatedLifecycleArtifactsV3,
       RecoverPredecessorLifecycleV3,
       PublishRestartRestoreSuccessorLifecycleStateSlotV3,
       PublishSuccessorLifecycleStateSlotV3WithAuthority,
       SyncSuccessorLifecycleStateDirectoryV3,
       ReplaceSuccessorLifecycleRootV3,
       CommitSuccessorLifecycleRootV3,
       PublishCommittedLifecycleV3ToMemory

THEOREM RestartRestorePending2EnablesRootResync ==
  RestartRestorePending2 =>
    ENABLED <<ResyncValidatedLifecycleRootDirectoryV3>>_typedRolloverVars
BY ExpandENABLED, IsaT(300)
   DEF RestartRestorePending2, RestartRestoreStage2,
       RestartRestoreStage3, RestartRestoreCorridorBase,
       RestartRestoreGoal,
       ResyncValidatedLifecycleRootDirectoryV3, typedRolloverVars

THEOREM RestartRestoreRootResyncExitsPending2 ==
  /\ RestartRestorePending2
  /\ <<ResyncValidatedLifecycleRootDirectoryV3>>_typedRolloverVars
  => RestartRestoreStage3'
BY ResponsiveRestartRestoreStepPreservesCorridorBase, IsaT(300)
   DEF RestartRestorePending2, RestartRestoreStage2,
       RestartRestoreStage3, RestartRestoreGoal,
       ResyncValidatedLifecycleRootDirectoryV3,
       typedRolloverVars, ResponsiveRestartRestoreNext

THEOREM RestartRestoreStage2LeadsToStage3 ==
  ResponsiveRestartRestoreSpec =>
    (RestartRestoreStage2 ~> RestartRestoreStage3)
PROOF
  <1>1. /\ RestartRestorePending2
         /\ [ResponsiveRestartRestoreNext]_typedRolloverVars
         => \/ RestartRestorePending2'
            \/ RestartRestoreStage3'
    BY RestartRestorePending2IsNotOrphaned
  <1>2. RestartRestorePending2 =>
          ENABLED
            <<ResyncValidatedLifecycleRootDirectoryV3>>_typedRolloverVars
    BY RestartRestorePending2EnablesRootResync
  <1>3. /\ RestartRestorePending2
         /\ <<ResyncValidatedLifecycleRootDirectoryV3>>_typedRolloverVars
         => RestartRestoreStage3'
    BY RestartRestoreRootResyncExitsPending2
  <1>4. ResponsiveRestartRestoreSpec =>
          WF_typedRolloverVars(
            ResyncValidatedLifecycleRootDirectoryV3)
    BY DEF ResponsiveRestartRestoreSpec
  <1>5. ResponsiveRestartRestoreSpec =>
          [][ResponsiveRestartRestoreNext]_typedRolloverVars
    BY DEF ResponsiveRestartRestoreSpec
  <1>6. ResponsiveRestartRestoreSpec =>
          [](RestartRestorePending2 /\ ~RestartRestoreStage3
             => ENABLED
                  <<ResyncValidatedLifecycleRootDirectoryV3>>_
                    typedRolloverVars)
    BY <1>2, PTL
  <1>7. ResponsiveRestartRestoreSpec =>
          [](RestartRestorePending2 /\ ~RestartRestoreStage3
             /\ <<ResyncValidatedLifecycleRootDirectoryV3>>_
                   typedRolloverVars
             => RestartRestoreStage3')
    BY <1>3, PTL
  <1>8. ResponsiveRestartRestoreSpec =>
          [](RestartRestorePending2 /\ ~RestartRestoreStage3
             /\ [ResponsiveRestartRestoreNext]_typedRolloverVars
             => \/ RestartRestoreStage3'
                \/ RestartRestorePending2')
    BY <1>1, PTL
  <1> QED BY <1>4, <1>5, <1>6, <1>7, <1>8, PTL
       DEF RestartRestorePending2

THEOREM RestartRestorePending3IsNotOrphaned ==
  /\ RestartRestorePending3
  /\ [ResponsiveRestartRestoreNext]_typedRolloverVars
  => \/ RestartRestorePending3'
     \/ RestartRestoreStage4'
BY ResponsiveRestartRestoreStepPreservesCorridorBase, IsaT(600)
   DEF RestartRestorePending3, RestartRestoreStage3,
       RestartRestoreStage4, RestartRestoreCorridorBase,
       RestartRestoreGoal, ResponsiveRestartRestoreNext,
       ValidateRootSelectedLifecycleV3,
       ResyncValidatedLifecycleStateDirectoryV3,
       ResyncValidatedLifecycleRootDirectoryV3,
       CleanupValidatedLifecycleArtifactsV3,
       RecoverPredecessorLifecycleV3,
       PublishRestartRestoreSuccessorLifecycleStateSlotV3,
       PublishSuccessorLifecycleStateSlotV3WithAuthority,
       SyncSuccessorLifecycleStateDirectoryV3,
       ReplaceSuccessorLifecycleRootV3,
       CommitSuccessorLifecycleRootV3,
       PublishCommittedLifecycleV3ToMemory

THEOREM RestartRestorePending3EnablesCleanup ==
  RestartRestorePending3 =>
    ENABLED <<CleanupValidatedLifecycleArtifactsV3>>_typedRolloverVars
BY ExpandENABLED, IsaT(600)
   DEF RestartRestorePending3, RestartRestoreStage3,
       RestartRestoreStage4, RestartRestoreCorridorBase,
       RestartRestoreGoal, TypedRolloverSafetyInvariant,
       RootAnchoredLifecycleV3Invariant,
       RootSelectedLifecyclePairMatches,
       LifecycleSnapshotSemanticallyValid, DurableSnapshot,
       LifecycleStateDirectoryIsSynced,
       LifecycleRootDirectoryIsSynced,
       CleanupValidatedLifecycleArtifactsV3, typedRolloverVars

THEOREM RestartRestoreCleanupExitsPending3 ==
  /\ RestartRestorePending3
  /\ <<CleanupValidatedLifecycleArtifactsV3>>_typedRolloverVars
  => RestartRestoreStage4'
BY ResponsiveRestartRestoreStepPreservesCorridorBase, IsaT(600)
   DEF RestartRestorePending3, RestartRestoreStage3,
       RestartRestoreStage4, RestartRestoreGoal,
       CleanupValidatedLifecycleArtifactsV3,
       typedRolloverVars, ResponsiveRestartRestoreNext

THEOREM RestartRestoreStage3LeadsToStage4 ==
  ResponsiveRestartRestoreSpec =>
    (RestartRestoreStage3 ~> RestartRestoreStage4)
PROOF
  <1>1. /\ RestartRestorePending3
         /\ [ResponsiveRestartRestoreNext]_typedRolloverVars
         => \/ RestartRestorePending3'
            \/ RestartRestoreStage4'
    BY RestartRestorePending3IsNotOrphaned
  <1>2. RestartRestorePending3 =>
          ENABLED <<CleanupValidatedLifecycleArtifactsV3>>_typedRolloverVars
    BY RestartRestorePending3EnablesCleanup
  <1>3. /\ RestartRestorePending3
         /\ <<CleanupValidatedLifecycleArtifactsV3>>_typedRolloverVars
         => RestartRestoreStage4'
    BY RestartRestoreCleanupExitsPending3
  <1>4. ResponsiveRestartRestoreSpec =>
          WF_typedRolloverVars(CleanupValidatedLifecycleArtifactsV3)
    BY DEF ResponsiveRestartRestoreSpec
  <1>5. ResponsiveRestartRestoreSpec =>
          [][ResponsiveRestartRestoreNext]_typedRolloverVars
    BY DEF ResponsiveRestartRestoreSpec
  <1>6. ResponsiveRestartRestoreSpec =>
          [](RestartRestorePending3 /\ ~RestartRestoreStage4
             => ENABLED
                  <<CleanupValidatedLifecycleArtifactsV3>>_
                    typedRolloverVars)
    BY <1>2, PTL
  <1>7. ResponsiveRestartRestoreSpec =>
          [](RestartRestorePending3 /\ ~RestartRestoreStage4
             /\ <<CleanupValidatedLifecycleArtifactsV3>>_
                   typedRolloverVars
             => RestartRestoreStage4')
    BY <1>3, PTL
  <1>8. ResponsiveRestartRestoreSpec =>
          [](RestartRestorePending3 /\ ~RestartRestoreStage4
             /\ [ResponsiveRestartRestoreNext]_typedRolloverVars
             => \/ RestartRestoreStage4'
                \/ RestartRestorePending3')
    BY <1>1, PTL
  <1> QED BY <1>4, <1>5, <1>6, <1>7, <1>8, PTL
       DEF RestartRestorePending3

THEOREM RestartRestorePending4IsNotOrphaned ==
  /\ RestartRestorePending4
  /\ [ResponsiveRestartRestoreNext]_typedRolloverVars
  => \/ RestartRestorePending4'
     \/ RestartRestoreStage5'
BY ResponsiveRestartRestoreStepPreservesCorridorBase, IsaT(600)
   DEF RestartRestorePending4, RestartRestoreStage4,
       RestartRestoreStage5, RestartRestoreCorridorBase,
       RestartRestoreGoal, ResponsiveRestartRestoreNext,
       ValidateRootSelectedLifecycleV3,
       ResyncValidatedLifecycleStateDirectoryV3,
       ResyncValidatedLifecycleRootDirectoryV3,
       CleanupValidatedLifecycleArtifactsV3,
       RecoverPredecessorLifecycleV3,
       PublishRestartRestoreSuccessorLifecycleStateSlotV3,
       PublishSuccessorLifecycleStateSlotV3WithAuthority,
       SyncSuccessorLifecycleStateDirectoryV3,
       ReplaceSuccessorLifecycleRootV3,
       CommitSuccessorLifecycleRootV3,
       PublishCommittedLifecycleV3ToMemory

THEOREM RestartRestorePending4EnablesRecovery ==
  RestartRestorePending4 =>
    ENABLED <<RecoverPredecessorLifecycleV3>>_typedRolloverVars
BY ExpandENABLED, IsaT(600)
   DEF RestartRestorePending4, RestartRestoreStage4,
       RestartRestoreStage5, RestartRestoreCorridorBase,
       RestartRestoreGoal, TypedRolloverSafetyInvariant,
       RootAnchoredLifecycleV3Invariant,
       RootSelectedLifecyclePairMatches,
       LifecycleSnapshotSemanticallyValid, DurableSnapshot,
       LifecycleMemoryMatchesDurableSnapshotV3,
       RecoverPredecessorLifecycleV3, typedRolloverVars

THEOREM RestartRestoreRecoveryExitsPending4 ==
  /\ RestartRestorePending4
  /\ <<RecoverPredecessorLifecycleV3>>_typedRolloverVars
  => RestartRestoreStage5'
BY ResponsiveRestartRestoreStepPreservesCorridorBase, IsaT(600)
   DEF RestartRestorePending4, RestartRestoreStage4,
       RestartRestoreStage5, RestartRestoreGoal,
       RecoverPredecessorLifecycleV3,
       typedRolloverVars, ResponsiveRestartRestoreNext

THEOREM RestartRestoreStage4LeadsToStage5 ==
  ResponsiveRestartRestoreSpec =>
    (RestartRestoreStage4 ~> RestartRestoreStage5)
PROOF
  <1>1. /\ RestartRestorePending4
         /\ [ResponsiveRestartRestoreNext]_typedRolloverVars
         => \/ RestartRestorePending4'
            \/ RestartRestoreStage5'
    BY RestartRestorePending4IsNotOrphaned
  <1>2. RestartRestorePending4 =>
          ENABLED <<RecoverPredecessorLifecycleV3>>_typedRolloverVars
    BY RestartRestorePending4EnablesRecovery
  <1>3. /\ RestartRestorePending4
         /\ <<RecoverPredecessorLifecycleV3>>_typedRolloverVars
         => RestartRestoreStage5'
    BY RestartRestoreRecoveryExitsPending4
  <1>4. ResponsiveRestartRestoreSpec =>
          WF_typedRolloverVars(RecoverPredecessorLifecycleV3)
    BY DEF ResponsiveRestartRestoreSpec
  <1>5. ResponsiveRestartRestoreSpec =>
          [][ResponsiveRestartRestoreNext]_typedRolloverVars
    BY DEF ResponsiveRestartRestoreSpec
  <1>6. ResponsiveRestartRestoreSpec =>
          [](RestartRestorePending4 /\ ~RestartRestoreStage5
             => ENABLED
                  <<RecoverPredecessorLifecycleV3>>_typedRolloverVars)
    BY <1>2, PTL
  <1>7. ResponsiveRestartRestoreSpec =>
          [](RestartRestorePending4 /\ ~RestartRestoreStage5
             /\ <<RecoverPredecessorLifecycleV3>>_typedRolloverVars
             => RestartRestoreStage5')
    BY <1>3, PTL
  <1>8. ResponsiveRestartRestoreSpec =>
          [](RestartRestorePending4 /\ ~RestartRestoreStage5
             /\ [ResponsiveRestartRestoreNext]_typedRolloverVars
             => \/ RestartRestoreStage5'
                \/ RestartRestorePending4')
    BY <1>1, PTL
  <1> QED BY <1>4, <1>5, <1>6, <1>7, <1>8, PTL
       DEF RestartRestorePending4

THEOREM RestartRestorePending5IsNotOrphaned ==
  /\ RestartRestorePending5
  /\ [ResponsiveRestartRestoreNext]_typedRolloverVars
  => \/ RestartRestorePending5'
     \/ RestartRestoreStage6'
BY ResponsiveRestartRestoreStepPreservesCorridorBase, IsaT(600)
   DEF RestartRestorePending5, RestartRestoreStage5,
       RestartRestoreStage6, RestartRestoreCorridorBase,
       RestartRestoreGoal, ResponsiveRestartRestoreNext,
       ValidateRootSelectedLifecycleV3,
       ResyncValidatedLifecycleStateDirectoryV3,
       ResyncValidatedLifecycleRootDirectoryV3,
       CleanupValidatedLifecycleArtifactsV3,
       RecoverPredecessorLifecycleV3,
       PublishRestartRestoreSuccessorLifecycleStateSlotV3,
       PublishSuccessorLifecycleStateSlotV3WithAuthority,
       SyncSuccessorLifecycleStateDirectoryV3,
       ReplaceSuccessorLifecycleRootV3,
       CommitSuccessorLifecycleRootV3,
       PublishCommittedLifecycleV3ToMemory

THEOREM RestartRestorePending5EnablesStateSlotPublish ==
  RestartRestorePending5 =>
    ENABLED
      <<PublishRestartRestoreSuccessorLifecycleStateSlotV3>>_
        typedRolloverVars
BY ExpandENABLED, IsaT(600)
   DEF RestartRestorePending5, RestartRestoreStage5,
       RestartRestoreStage6, RestartRestoreCorridorBase,
       RestartRestoreGoal, ChangedRosterReplacementNeeded,
       ChangedRosterAuthorityAvailable,
       PublishRestartRestoreSuccessorLifecycleStateSlotV3,
       PublishSuccessorLifecycleStateSlotV3WithAuthority,
       LifecycleJournalReady, LifecycleMemoryMatchesDurableSnapshotV3,
       typedRolloverVars

THEOREM RestartRestoreStateSlotPublishExitsPending5 ==
  /\ RestartRestorePending5
  /\ <<PublishRestartRestoreSuccessorLifecycleStateSlotV3>>_
       typedRolloverVars
  => RestartRestoreStage6'
BY ResponsiveRestartRestoreStepPreservesCorridorBase, IsaT(600)
   DEF RestartRestorePending5, RestartRestoreStage5,
       RestartRestoreStage6, RestartRestoreGoal,
       PublishRestartRestoreSuccessorLifecycleStateSlotV3,
       PublishSuccessorLifecycleStateSlotV3WithAuthority,
       LifecycleSnapshotV3, typedRolloverVars,
       ResponsiveRestartRestoreNext

THEOREM RestartRestoreStage5LeadsToStage6 ==
  ResponsiveRestartRestoreSpec =>
    (RestartRestoreStage5 ~> RestartRestoreStage6)
PROOF
  <1>1. /\ RestartRestorePending5
         /\ [ResponsiveRestartRestoreNext]_typedRolloverVars
         => \/ RestartRestorePending5'
            \/ RestartRestoreStage6'
    BY RestartRestorePending5IsNotOrphaned
  <1>2. RestartRestorePending5 =>
          ENABLED
            <<PublishRestartRestoreSuccessorLifecycleStateSlotV3>>_
              typedRolloverVars
    BY RestartRestorePending5EnablesStateSlotPublish
  <1>3. /\ RestartRestorePending5
         /\ <<PublishRestartRestoreSuccessorLifecycleStateSlotV3>>_
              typedRolloverVars
         => RestartRestoreStage6'
    BY RestartRestoreStateSlotPublishExitsPending5
  <1>4. ResponsiveRestartRestoreSpec =>
          WF_typedRolloverVars(
            PublishRestartRestoreSuccessorLifecycleStateSlotV3)
    BY DEF ResponsiveRestartRestoreSpec
  <1>5. ResponsiveRestartRestoreSpec =>
          [][ResponsiveRestartRestoreNext]_typedRolloverVars
    BY DEF ResponsiveRestartRestoreSpec
  <1>6. ResponsiveRestartRestoreSpec =>
          [](RestartRestorePending5 /\ ~RestartRestoreStage6
             => ENABLED
                  <<PublishRestartRestoreSuccessorLifecycleStateSlotV3>>_
                    typedRolloverVars)
    BY <1>2, PTL
  <1>7. ResponsiveRestartRestoreSpec =>
          [](RestartRestorePending5 /\ ~RestartRestoreStage6
             /\ <<PublishRestartRestoreSuccessorLifecycleStateSlotV3>>_
                   typedRolloverVars
             => RestartRestoreStage6')
    BY <1>3, PTL
  <1>8. ResponsiveRestartRestoreSpec =>
          [](RestartRestorePending5 /\ ~RestartRestoreStage6
             /\ [ResponsiveRestartRestoreNext]_typedRolloverVars
             => \/ RestartRestoreStage6'
                \/ RestartRestorePending5')
    BY <1>1, PTL
  <1> QED BY <1>4, <1>5, <1>6, <1>7, <1>8, PTL
       DEF RestartRestorePending5

THEOREM RestartRestorePending6IsNotOrphaned ==
  /\ RestartRestorePending6
  /\ [ResponsiveRestartRestoreNext]_typedRolloverVars
  => \/ RestartRestorePending6'
     \/ RestartRestoreStage7'
BY ResponsiveRestartRestoreStepPreservesCorridorBase, IsaT(600)
   DEF RestartRestorePending6, RestartRestoreStage6,
       RestartRestoreStage7, RestartRestoreCorridorBase,
       RestartRestoreGoal, ResponsiveRestartRestoreNext,
       ValidateRootSelectedLifecycleV3,
       ResyncValidatedLifecycleStateDirectoryV3,
       ResyncValidatedLifecycleRootDirectoryV3,
       CleanupValidatedLifecycleArtifactsV3,
       RecoverPredecessorLifecycleV3,
       PublishRestartRestoreSuccessorLifecycleStateSlotV3,
       PublishSuccessorLifecycleStateSlotV3WithAuthority,
       SyncSuccessorLifecycleStateDirectoryV3,
       ReplaceSuccessorLifecycleRootV3,
       CommitSuccessorLifecycleRootV3,
       PublishCommittedLifecycleV3ToMemory

THEOREM RestartRestorePending6EnablesStateDirectorySync ==
  RestartRestorePending6 =>
    ENABLED <<SyncSuccessorLifecycleStateDirectoryV3>>_typedRolloverVars
BY ExpandENABLED, IsaT(600)
   DEF RestartRestorePending6, RestartRestoreStage6,
       RestartRestoreStage7, RestartRestoreCorridorBase,
       RestartRestoreGoal, TypedRolloverSafetyInvariant,
       LifecycleCommitPhaseInvariant,
       ValidatedCandidateSuccessorStateSlotAheadOfRoot,
       DurableCandidateStateSlotAheadOfRoot,
       LifecycleStateDirectoryIsSynced,
       SyncSuccessorLifecycleStateDirectoryV3, typedRolloverVars

THEOREM RestartRestoreStateDirectorySyncExitsPending6 ==
  /\ RestartRestorePending6
  /\ <<SyncSuccessorLifecycleStateDirectoryV3>>_typedRolloverVars
  => RestartRestoreStage7'
BY ResponsiveRestartRestoreStepPreservesCorridorBase, IsaT(600)
   DEF RestartRestorePending6, RestartRestoreStage6,
       RestartRestoreStage7, RestartRestoreGoal,
       SyncSuccessorLifecycleStateDirectoryV3,
       typedRolloverVars, ResponsiveRestartRestoreNext

THEOREM RestartRestoreStage6LeadsToStage7 ==
  ResponsiveRestartRestoreSpec =>
    (RestartRestoreStage6 ~> RestartRestoreStage7)
PROOF
  <1>1. /\ RestartRestorePending6
         /\ [ResponsiveRestartRestoreNext]_typedRolloverVars
         => \/ RestartRestorePending6'
            \/ RestartRestoreStage7'
    BY RestartRestorePending6IsNotOrphaned
  <1>2. RestartRestorePending6 =>
          ENABLED
            <<SyncSuccessorLifecycleStateDirectoryV3>>_typedRolloverVars
    BY RestartRestorePending6EnablesStateDirectorySync
  <1>3. /\ RestartRestorePending6
         /\ <<SyncSuccessorLifecycleStateDirectoryV3>>_typedRolloverVars
         => RestartRestoreStage7'
    BY RestartRestoreStateDirectorySyncExitsPending6
  <1>4. ResponsiveRestartRestoreSpec =>
          WF_typedRolloverVars(SyncSuccessorLifecycleStateDirectoryV3)
    BY DEF ResponsiveRestartRestoreSpec
  <1>5. ResponsiveRestartRestoreSpec =>
          [][ResponsiveRestartRestoreNext]_typedRolloverVars
    BY DEF ResponsiveRestartRestoreSpec
  <1>6. ResponsiveRestartRestoreSpec =>
          [](RestartRestorePending6 /\ ~RestartRestoreStage7
             => ENABLED
                  <<SyncSuccessorLifecycleStateDirectoryV3>>_
                    typedRolloverVars)
    BY <1>2, PTL
  <1>7. ResponsiveRestartRestoreSpec =>
          [](RestartRestorePending6 /\ ~RestartRestoreStage7
             /\ <<SyncSuccessorLifecycleStateDirectoryV3>>_
                   typedRolloverVars
             => RestartRestoreStage7')
    BY <1>3, PTL
  <1>8. ResponsiveRestartRestoreSpec =>
          [](RestartRestorePending6 /\ ~RestartRestoreStage7
             /\ [ResponsiveRestartRestoreNext]_typedRolloverVars
             => \/ RestartRestoreStage7'
                \/ RestartRestorePending6')
    BY <1>1, PTL
  <1> QED BY <1>4, <1>5, <1>6, <1>7, <1>8, PTL
       DEF RestartRestorePending6

THEOREM RestartRestorePending7IsNotOrphaned ==
  /\ RestartRestorePending7
  /\ [ResponsiveRestartRestoreNext]_typedRolloverVars
  => \/ RestartRestorePending7'
     \/ RestartRestoreStage8'
BY ResponsiveRestartRestoreStepPreservesCorridorBase, IsaT(600)
   DEF RestartRestorePending7, RestartRestoreStage7,
       RestartRestoreStage8, RestartRestoreCorridorBase,
       RestartRestoreGoal, ResponsiveRestartRestoreNext,
       ValidateRootSelectedLifecycleV3,
       ResyncValidatedLifecycleStateDirectoryV3,
       ResyncValidatedLifecycleRootDirectoryV3,
       CleanupValidatedLifecycleArtifactsV3,
       RecoverPredecessorLifecycleV3,
       PublishRestartRestoreSuccessorLifecycleStateSlotV3,
       PublishSuccessorLifecycleStateSlotV3WithAuthority,
       SyncSuccessorLifecycleStateDirectoryV3,
       ReplaceSuccessorLifecycleRootV3,
       CommitSuccessorLifecycleRootV3,
       PublishCommittedLifecycleV3ToMemory

THEOREM RestartRestorePending7EnablesRootReplacement ==
  RestartRestorePending7 =>
    ENABLED <<ReplaceSuccessorLifecycleRootV3>>_typedRolloverVars
BY ExpandENABLED, IsaT(600)
   DEF RestartRestorePending7, RestartRestoreStage7,
       RestartRestoreStage8, RestartRestoreCorridorBase,
       RestartRestoreGoal, TypedRolloverSafetyInvariant,
       LifecycleCommitPhaseInvariant,
       DurableCandidateStateSlotAheadOfRoot,
       LifecycleStateDirectoryIsSynced,
       LifecycleRootDirectoryIsSynced,
       ReplaceSuccessorLifecycleRootV3, typedRolloverVars

THEOREM RestartRestoreRootReplacementExitsPending7 ==
  /\ RestartRestorePending7
  /\ <<ReplaceSuccessorLifecycleRootV3>>_typedRolloverVars
  => RestartRestoreStage8'
BY ResponsiveRestartRestoreStepPreservesCorridorBase, IsaT(600)
   DEF RestartRestorePending7, RestartRestoreStage7,
       RestartRestoreStage8, RestartRestoreGoal,
       ReplaceSuccessorLifecycleRootV3,
       typedRolloverVars, ResponsiveRestartRestoreNext

THEOREM RestartRestoreStage7LeadsToStage8 ==
  ResponsiveRestartRestoreSpec =>
    (RestartRestoreStage7 ~> RestartRestoreStage8)
PROOF
  <1>1. /\ RestartRestorePending7
         /\ [ResponsiveRestartRestoreNext]_typedRolloverVars
         => \/ RestartRestorePending7'
            \/ RestartRestoreStage8'
    BY RestartRestorePending7IsNotOrphaned
  <1>2. RestartRestorePending7 =>
          ENABLED <<ReplaceSuccessorLifecycleRootV3>>_typedRolloverVars
    BY RestartRestorePending7EnablesRootReplacement
  <1>3. /\ RestartRestorePending7
         /\ <<ReplaceSuccessorLifecycleRootV3>>_typedRolloverVars
         => RestartRestoreStage8'
    BY RestartRestoreRootReplacementExitsPending7
  <1>4. ResponsiveRestartRestoreSpec =>
          WF_typedRolloverVars(ReplaceSuccessorLifecycleRootV3)
    BY DEF ResponsiveRestartRestoreSpec
  <1>5. ResponsiveRestartRestoreSpec =>
          [][ResponsiveRestartRestoreNext]_typedRolloverVars
    BY DEF ResponsiveRestartRestoreSpec
  <1>6. ResponsiveRestartRestoreSpec =>
          [](RestartRestorePending7 /\ ~RestartRestoreStage8
             => ENABLED
                  <<ReplaceSuccessorLifecycleRootV3>>_typedRolloverVars)
    BY <1>2, PTL
  <1>7. ResponsiveRestartRestoreSpec =>
          [](RestartRestorePending7 /\ ~RestartRestoreStage8
             /\ <<ReplaceSuccessorLifecycleRootV3>>_typedRolloverVars
             => RestartRestoreStage8')
    BY <1>3, PTL
  <1>8. ResponsiveRestartRestoreSpec =>
          [](RestartRestorePending7 /\ ~RestartRestoreStage8
             /\ [ResponsiveRestartRestoreNext]_typedRolloverVars
             => \/ RestartRestoreStage8'
                \/ RestartRestorePending7')
    BY <1>1, PTL
  <1> QED BY <1>4, <1>5, <1>6, <1>7, <1>8, PTL
       DEF RestartRestorePending7

THEOREM RestartRestorePending8IsNotOrphaned ==
  /\ RestartRestorePending8
  /\ [ResponsiveRestartRestoreNext]_typedRolloverVars
  => \/ RestartRestorePending8'
     \/ RestartRestoreStage9'
BY ResponsiveRestartRestoreStepPreservesCorridorBase, IsaT(600)
   DEF RestartRestorePending8, RestartRestoreStage8,
       RestartRestoreStage9, RestartRestoreCorridorBase,
       RestartRestoreGoal, ResponsiveRestartRestoreNext,
       ValidateRootSelectedLifecycleV3,
       ResyncValidatedLifecycleStateDirectoryV3,
       ResyncValidatedLifecycleRootDirectoryV3,
       CleanupValidatedLifecycleArtifactsV3,
       RecoverPredecessorLifecycleV3,
       PublishRestartRestoreSuccessorLifecycleStateSlotV3,
       PublishSuccessorLifecycleStateSlotV3WithAuthority,
       SyncSuccessorLifecycleStateDirectoryV3,
       ReplaceSuccessorLifecycleRootV3,
       CommitSuccessorLifecycleRootV3,
       PublishCommittedLifecycleV3ToMemory

THEOREM RestartRestorePending8EnablesRootCommit ==
  RestartRestorePending8 =>
    ENABLED <<CommitSuccessorLifecycleRootV3>>_typedRolloverVars
BY ExpandENABLED, IsaT(600)
   DEF RestartRestorePending8, RestartRestoreStage8,
       RestartRestoreStage9, RestartRestoreCorridorBase,
       RestartRestoreGoal, TypedRolloverSafetyInvariant,
       LifecycleCommitPhaseInvariant,
       RootSelectedSuccessorAheadOfMemory,
       RootSelectedLifecyclePairMatches,
       LifecycleSnapshotSemanticallyValid,
       LifecycleStateDirectoryIsSynced,
       LifecycleRootDirectoryIsSynced,
       CommitSuccessorLifecycleRootV3, typedRolloverVars

THEOREM RestartRestoreRootCommitExitsPending8 ==
  /\ RestartRestorePending8
  /\ <<CommitSuccessorLifecycleRootV3>>_typedRolloverVars
  => RestartRestoreStage9'
BY ResponsiveRestartRestoreStepPreservesCorridorBase, IsaT(600)
   DEF RestartRestorePending8, RestartRestoreStage8,
       RestartRestoreStage9, RestartRestoreGoal,
       CommitSuccessorLifecycleRootV3,
       typedRolloverVars, ResponsiveRestartRestoreNext

THEOREM RestartRestoreStage8LeadsToStage9 ==
  ResponsiveRestartRestoreSpec =>
    (RestartRestoreStage8 ~> RestartRestoreStage9)
PROOF
  <1>1. /\ RestartRestorePending8
         /\ [ResponsiveRestartRestoreNext]_typedRolloverVars
         => \/ RestartRestorePending8'
            \/ RestartRestoreStage9'
    BY RestartRestorePending8IsNotOrphaned
  <1>2. RestartRestorePending8 =>
          ENABLED <<CommitSuccessorLifecycleRootV3>>_typedRolloverVars
    BY RestartRestorePending8EnablesRootCommit
  <1>3. /\ RestartRestorePending8
         /\ <<CommitSuccessorLifecycleRootV3>>_typedRolloverVars
         => RestartRestoreStage9'
    BY RestartRestoreRootCommitExitsPending8
  <1>4. ResponsiveRestartRestoreSpec =>
          WF_typedRolloverVars(CommitSuccessorLifecycleRootV3)
    BY DEF ResponsiveRestartRestoreSpec
  <1>5. ResponsiveRestartRestoreSpec =>
          [][ResponsiveRestartRestoreNext]_typedRolloverVars
    BY DEF ResponsiveRestartRestoreSpec
  <1>6. ResponsiveRestartRestoreSpec =>
          [](RestartRestorePending8 /\ ~RestartRestoreStage9
             => ENABLED
                  <<CommitSuccessorLifecycleRootV3>>_typedRolloverVars)
    BY <1>2, PTL
  <1>7. ResponsiveRestartRestoreSpec =>
          [](RestartRestorePending8 /\ ~RestartRestoreStage9
             /\ <<CommitSuccessorLifecycleRootV3>>_typedRolloverVars
             => RestartRestoreStage9')
    BY <1>3, PTL
  <1>8. ResponsiveRestartRestoreSpec =>
          [](RestartRestorePending8 /\ ~RestartRestoreStage9
             /\ [ResponsiveRestartRestoreNext]_typedRolloverVars
             => \/ RestartRestoreStage9'
                \/ RestartRestorePending8')
    BY <1>1, PTL
  <1> QED BY <1>4, <1>5, <1>6, <1>7, <1>8, PTL
       DEF RestartRestorePending8

THEOREM RestartRestorePending9IsNotOrphaned ==
  /\ RestartRestorePending9
  /\ [ResponsiveRestartRestoreNext]_typedRolloverVars
  => \/ RestartRestorePending9'
     \/ RestartRestoreGoal'
BY ResponsiveRestartRestoreStepPreservesCorridorBase, IsaT(600)
   DEF RestartRestorePending9, RestartRestoreStage9,
       RestartRestoreCorridorBase, RestartRestoreGoal,
       RestartRestoreSuccessorActiveWithoutRestart,
       ResponsiveRestartRestoreNext,
       ValidateRootSelectedLifecycleV3,
       ResyncValidatedLifecycleStateDirectoryV3,
       ResyncValidatedLifecycleRootDirectoryV3,
       CleanupValidatedLifecycleArtifactsV3,
       RecoverPredecessorLifecycleV3,
       PublishRestartRestoreSuccessorLifecycleStateSlotV3,
       PublishSuccessorLifecycleStateSlotV3WithAuthority,
       SyncSuccessorLifecycleStateDirectoryV3,
       ReplaceSuccessorLifecycleRootV3,
       CommitSuccessorLifecycleRootV3,
       PublishCommittedLifecycleV3ToMemory

THEOREM RestartRestorePending9EnablesMemoryPublication ==
  RestartRestorePending9 =>
    ENABLED <<PublishCommittedLifecycleV3ToMemory>>_typedRolloverVars
BY ExpandENABLED, IsaT(600)
   DEF RestartRestorePending9, RestartRestoreStage9,
       RestartRestoreCorridorBase, RestartRestoreGoal,
       TypedRolloverSafetyInvariant, LifecycleCommitPhaseInvariant,
       RootCommittedSuccessorAheadOfMemory,
       PublishCommittedLifecycleV3ToMemory, typedRolloverVars

THEOREM RestartRestoreMemoryPublicationExitsPending9 ==
  /\ RestartRestorePending9
  /\ <<PublishCommittedLifecycleV3ToMemory>>_typedRolloverVars
  => RestartRestoreGoal'
BY ResponsiveRestartRestoreStepPreservesCorridorBase, IsaT(600)
   DEF RestartRestorePending9, RestartRestoreStage9,
       RestartRestoreGoal,
       RestartRestoreSuccessorActiveWithoutRestart,
       PublishCommittedLifecycleV3ToMemory,
       typedRolloverVars, ResponsiveRestartRestoreNext

THEOREM RestartRestoreStage9LeadsToGoal ==
  ResponsiveRestartRestoreSpec =>
    (RestartRestoreStage9 ~> RestartRestoreGoal)
PROOF
  <1>1. /\ RestartRestorePending9
         /\ [ResponsiveRestartRestoreNext]_typedRolloverVars
         => \/ RestartRestorePending9'
            \/ RestartRestoreGoal'
    BY RestartRestorePending9IsNotOrphaned
  <1>2. RestartRestorePending9 =>
          ENABLED <<PublishCommittedLifecycleV3ToMemory>>_typedRolloverVars
    BY RestartRestorePending9EnablesMemoryPublication
  <1>3. /\ RestartRestorePending9
         /\ <<PublishCommittedLifecycleV3ToMemory>>_typedRolloverVars
         => RestartRestoreGoal'
    BY RestartRestoreMemoryPublicationExitsPending9
  <1>4. ResponsiveRestartRestoreSpec =>
          WF_typedRolloverVars(PublishCommittedLifecycleV3ToMemory)
    BY DEF ResponsiveRestartRestoreSpec
  <1>5. ResponsiveRestartRestoreSpec =>
          [][ResponsiveRestartRestoreNext]_typedRolloverVars
    BY DEF ResponsiveRestartRestoreSpec
  <1>6. ResponsiveRestartRestoreSpec =>
          [](RestartRestorePending9 /\ ~RestartRestoreGoal
             => ENABLED
                  <<PublishCommittedLifecycleV3ToMemory>>_
                    typedRolloverVars)
    BY <1>2, PTL
  <1>7. ResponsiveRestartRestoreSpec =>
          [](RestartRestorePending9 /\ ~RestartRestoreGoal
             /\ <<PublishCommittedLifecycleV3ToMemory>>_typedRolloverVars
             => RestartRestoreGoal')
    BY <1>3, PTL
  <1>8. ResponsiveRestartRestoreSpec =>
          [](RestartRestorePending9 /\ ~RestartRestoreGoal
             /\ [ResponsiveRestartRestoreNext]_typedRolloverVars
             => \/ RestartRestoreGoal'
                \/ RestartRestorePending9')
    BY <1>1, PTL
  <1> QED BY <1>4, <1>5, <1>6, <1>7, <1>8, PTL
       DEF RestartRestorePending9

THEOREM ResponsiveRestartRestoreRolloverLivenessObligation ==
  ResponsiveRestartRestoreSpec =>
    ResponsiveRestartRestoreRolloverLiveness
PROOF
  <1>1. ASSUME ResponsiveRestartRestoreSpec
         PROVE state.restartRequired ~> RestartRestoreGoal
    <2>1. []RestartRestoreCorridorBase
      BY <1>1, ResponsiveRestartRestoreAlwaysCorridorBase
    <2>2. state.restartRequired ~> RestartRestoreStage0
      BY <2>1, PTL DEF RestartRestoreStage0
    <2>3. RestartRestoreStage0 ~> RestartRestoreStage1
      BY <1>1, RestartRestoreStage0LeadsToStage1
    <2>4. RestartRestoreStage1 ~> RestartRestoreStage2
      BY <1>1, RestartRestoreStage1LeadsToStage2
    <2>5. RestartRestoreStage2 ~> RestartRestoreStage3
      BY <1>1, RestartRestoreStage2LeadsToStage3
    <2>6. RestartRestoreStage3 ~> RestartRestoreStage4
      BY <1>1, RestartRestoreStage3LeadsToStage4
    <2>7. RestartRestoreStage4 ~> RestartRestoreStage5
      BY <1>1, RestartRestoreStage4LeadsToStage5
    <2>8. RestartRestoreStage5 ~> RestartRestoreStage6
      BY <1>1, RestartRestoreStage5LeadsToStage6
    <2>9. RestartRestoreStage6 ~> RestartRestoreStage7
      BY <1>1, RestartRestoreStage6LeadsToStage7
    <2>10. RestartRestoreStage7 ~> RestartRestoreStage8
      BY <1>1, RestartRestoreStage7LeadsToStage8
    <2>11. RestartRestoreStage8 ~> RestartRestoreStage9
      BY <1>1, RestartRestoreStage8LeadsToStage9
    <2>12. RestartRestoreStage9 ~> RestartRestoreGoal
      BY <1>1, RestartRestoreStage9LeadsToGoal
    <2> QED BY <2>2, <2>3, <2>4, <2>5, <2>6, <2>7,
                 <2>8, <2>9, <2>10, <2>11, <2>12, PTL
  <1> QED BY <1>1
       DEF ResponsiveRestartRestoreRolloverLiveness,
           RestartRestoreGoal

=============================================================================
