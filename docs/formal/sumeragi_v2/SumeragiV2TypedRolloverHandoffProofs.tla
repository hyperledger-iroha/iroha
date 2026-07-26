---- MODULE SumeragiV2TypedRolloverHandoffProofs ----
EXTENDS SumeragiV2TypedRolloverHandoff, TLAPS

(***************************************************************************
Deductive safety and conditional local-liveness boundary for the move-only
exact-output rollover handoff.

The induction proves the complete fixed-model invariant, including fail-stop
protocol rejection and the durable-high-water-ahead lifecycle-snapshot
midpoint. The temporal proof derives the executable model's changed-roster
local-liveness property from its ten weak-fair local service actions under
`NoRolloverFailure`. It does not import any network-delivery or writer-flush
premise and does not prove recovery after a fail-stop rejection.
***************************************************************************)

THEOREM TypedRolloverInitEstablishesSafety ==
  Init => TypedRolloverSafetyInvariant
BY SMTT(45)
   DEF Init, TypedRolloverSafetyInvariant,
       TypedRolloverTypeInvariant, ReceiptLifecycleInvariant,
       FinalSealRejectsLateEnqueueInvariant,
       ReceiptExactOwnerAndPredecessorInvariant,
       RetainedWrapperIdentityInvariant,
       TransitionAuthorityLifecycleInvariant,
       OrdinaryChangedRosterTransitionInvariant,
       LiveChangedRosterNeedsTypedReceiptInvariant,
       TypedTransitionAtomicClearInvariant,
       SameRosterTransportPreservationInvariant,
       SameRosterRetryPreservationInvariant,
       RetryChunkHasLiveOwnerInvariant,
       DurableHighWaterFailClosedInvariant,
       HighWaterAheadSnapshotInvariant,
       TornMidpointOpenRejectionInvariant,
       TornHighWaterHistoryOriginInvariant,
       ForeignOwnerCandidateRejectionInvariant,
       PredecessorMismatchCandidateRejectionInvariant,
       WrongSuccessorCandidateRejectionInvariant,
       FailureLatchInvariant,
       PredecessorMismatchFailure, ValidationCandidatePending,
       validationHistoryVars, validationHistoriesExceptTorn,
       validationHistoriesExceptForeignOwner,
       validationHistoriesExceptPredecessorCandidate,
       validationHistoriesExceptWrongSuccessorCandidate,
       LateOldCallbackIsolationInvariant,
       HighWaterAheadOfLifecycleSnapshot,
       PreparedHighWaterSnapshotMidpoint,
       TornHighWaterSnapshotMidpoint,
       ExactPredecessorReceipt, ExactServiceTransportOwnerPair,
       ExactSuccessorConstruction, RosterRelations, ReceiptStages,
       TransitionAuthorities, FailureReasons, OwnerNonces, PredecessorMismatchKinds, Contexts,
       Artifacts, Parents, Successors, NoIdentity, ServiceOwnerNonce,
       TransportOwnerNonce, ExpectedContext, ExpectedArtifact,
       ExpectedParent, ExpectedSuccessor, InitialWorkerOutstanding,
       InitialGeneration, NextGeneration

THEOREM FinalSealMintsExactOwnerBoundReceipt ==
  SealAppliedHeightOutputHandoff =>
    /\ finalityValidated
    /\ workerIngressClosed
    /\ workerOutstanding = 0
    /\ ownerSealed'
    /\ receiptStage' = "Minted"
    /\ receiptOwnerNonce' = ServiceOwnerNonce
    /\ receiptContext' = ExpectedContext
    /\ receiptArtifact' = ExpectedArtifact
BY DEF SealAppliedHeightOutputHandoff

THEOREM SharedOwnerNonceDistinguishesForeignService ==
  /\ ForeignOwnerNonce # ServiceOwnerNonce
  /\ ForeignOwnerNonce # TransportOwnerNonce
BY SMTT(10)
   DEF ForeignOwnerNonce, ServiceOwnerNonce, TransportOwnerNonce

THEOREM RetainedWrapperRequiresExactPairAndSuccessor ==
  ConsumeReceiptIntoRetainedMergeSidecars =>
    /\ ExactPredecessorReceipt
    /\ ExactSuccessorConstruction
    /\ receiptStage' = "Retained"
    /\ retainedSuccessor' = ExpectedSuccessor
BY DEF ConsumeReceiptIntoRetainedMergeSidecars

THEOREM OrdinaryLiveTransitionRejectionIsNonDestructive ==
  RejectOrdinaryLiveChangedRosterTransition =>
    /\ serviceGeneration' = serviceGeneration
    /\ durableHighWater' = durableHighWater
    /\ responderActive' = responderActive
    /\ responderWritable' = responderWritable
    /\ responderAuthorized' = responderAuthorized
    /\ retryableChunk' = retryableChunk
    /\ successorActive' = successorActive
    /\ receiptStage' = receiptStage
BY DEF RejectOrdinaryLiveChangedRosterTransition

THEOREM TypedTransitionConsumesAndAtomicallyClears ==
  TypedChangedRosterTransition =>
    /\ receiptStage' = "Consumed"
    /\ receiptConsumeCount' = 1
    /\ serviceGeneration' = NextGeneration
    /\ durableHighWater' = NextGeneration
    /\ ~responderActive'
    /\ ~responderWritable'
    /\ ~responderAuthorized'
    /\ retryableChunk' = 0
    /\ successorActive'
    /\ transitionAuthority' = "Typed"
    /\ forcedTransitionUsed'
BY DEF TypedChangedRosterTransition

THEOREM SameRosterConsumesAuthorityButPreservesRetry ==
  SameRosterRetainedTransportRollover =>
    /\ receiptStage' = "Consumed"
    /\ receiptConsumeCount' = 1
    /\ serviceGeneration' = serviceGeneration
    /\ durableHighWater' = durableHighWater
    /\ responderActive' = responderActive
    /\ responderWritable' = responderWritable
    /\ responderAuthorized' = responderAuthorized
    /\ retryableChunk' = retryableChunk
    /\ ~forcedTransitionUsed'
BY DEF SameRosterRetainedTransportRollover

THEOREM SealedCorridorLateEnqueueFailsStopWithoutOutputMutation ==
  RejectLateExactOutputEnqueue =>
    /\ workerOutstanding' = workerOutstanding
    /\ ownerSealed' = ownerSealed
    /\ receiptStage' = receiptStage
    /\ restartRequired'
    /\ ~successorActive'
    /\ failureReason' = "LateExactOutputEnqueue"
BY DEF RejectLateExactOutputEnqueue

THEOREM ForeignOwnerMismatchFailsStop ==
  RejectSameContextForeignOwnerReceipt =>
    /\ foreignReceiptRejected'
    /\ ~foreignReceiptCandidatePresent'
    /\ restartRequired'
    /\ ~successorActive'
    /\ failureReason' = "ForeignOwnerMismatch"
    /\ receiptStage' = receiptStage
BY DEF RejectSameContextForeignOwnerReceipt

THEOREM PredecessorMismatchFailsStop ==
  RejectMismatchedPredecessorReceipt =>
    /\ predecessorMismatchRejected'
    /\ predecessorMismatchCandidateKind' = "NoMismatch"
    /\ restartRequired'
    /\ ~successorActive'
    /\ failureReason' =
         PredecessorMismatchFailure(predecessorMismatchCandidateKind)
    /\ receiptStage' = receiptStage
BY DEF RejectMismatchedPredecessorReceipt

THEOREM WrongImmediateSuccessorFailsStop ==
  RejectWrongImmediateSuccessor =>
    /\ wrongSuccessorRejected'
    /\ ~wrongSuccessorCandidatePresent'
    /\ restartRequired'
    /\ ~successorActive'
    /\ failureReason' = "ImmediateSuccessorMismatch"
    /\ receiptStage' = receiptStage
BY DEF RejectWrongImmediateSuccessor

THEOREM LateOldCallbackCannotMutateSuccessor ==
  ObserveLateOldWriterCallback =>
    /\ successorCursor' = successorCursor
    /\ serviceGeneration' = serviceGeneration
    /\ durableHighWater' = durableHighWater
    /\ responderActive' = responderActive
    /\ responderWritable' = responderWritable
    /\ responderAuthorized' = responderAuthorized
    /\ retryableChunk' = retryableChunk
    /\ receiptStage' = receiptStage
    /\ receiptConsumeCount' = receiptConsumeCount
BY DEF ObserveLateOldWriterCallback

THEOREM RollbackMismatchFailsClosed ==
  CrashWithRolledBackHighWater =>
    /\ serviceGeneration' = NextGeneration
    /\ durableHighWater' = InitialGeneration
    /\ serviceGeneration' > durableHighWater'
    /\ ~successorActive'
    /\ restartRequired'
    /\ failureReason' = "RollbackMismatch"
BY SMTT(10)
   DEF CrashWithRolledBackHighWater,
       InitialGeneration, NextGeneration

THEOREM HighWaterPersistenceCreatesPreparedSnapshotMidpoint ==
  PersistNextServiceHighWater =>
    PreparedHighWaterSnapshotMidpoint'
BY SMTT(10)
   DEF PersistNextServiceHighWater,
       PreparedHighWaterSnapshotMidpoint,
       HighWaterAheadOfLifecycleSnapshot,
       InitialGeneration, NextGeneration

THEOREM LifecycleSnapshotFailureLeavesTornMidpointInactive ==
  FailLifecycleSnapshotAfterHighWaterPersistence =>
    /\ TornHighWaterSnapshotMidpoint'
    /\ tornHighWaterHistory'
    /\ restartRequired'
    /\ ~successorActive'
BY SMTT(10)
   DEF FailLifecycleSnapshotAfterHighWaterPersistence,
       PreparedHighWaterSnapshotMidpoint,
       TornHighWaterSnapshotMidpoint,
       HighWaterAheadOfLifecycleSnapshot

THEOREM MidpointCrashLeavesTornMidpointInactive ==
  CrashAtHighWaterAheadSnapshot =>
    /\ TornHighWaterSnapshotMidpoint'
    /\ tornHighWaterHistory'
    /\ restartRequired'
    /\ ~successorActive'
BY SMTT(10)
   DEF CrashAtHighWaterAheadSnapshot,
       PreparedHighWaterSnapshotMidpoint,
       TornHighWaterSnapshotMidpoint,
       HighWaterAheadOfLifecycleSnapshot

THEOREM TornMidpointOpenIsRejectedWithoutActivation ==
  RejectTornHighWaterSnapshotOpen =>
    /\ tornMidpointOpenRejected'
    /\ successorActive' = successorActive
    /\ serviceGeneration' = serviceGeneration
    /\ durableHighWater' = durableHighWater
    /\ restartRequired' = restartRequired
    /\ tornHighWaterHistory' = tornHighWaterHistory
BY DEF RejectTornHighWaterSnapshotOpen, validationHistoryVars

THEOREM TypedRolloverNextSatisfiesActionSafety ==
  /\ TypedRolloverSafetyInvariant
  /\ Next
  =>
    /\ TornHighWaterHistoryStepSafety
    /\ ForeignOwnerCandidateStepSafety
    /\ PredecessorMismatchCandidateStepSafety
    /\ WrongSuccessorCandidateStepSafety
BY SMTT(120)
   DEF TypedRolloverSafetyInvariant,
       DurableHighWaterFailClosedInvariant,
       ForeignOwnerCandidateRejectionInvariant,
       PredecessorMismatchCandidateRejectionInvariant,
       WrongSuccessorCandidateRejectionInvariant,
       TornHighWaterHistoryStepSafety,
       ForeignOwnerCandidateStepSafety,
       PredecessorMismatchCandidateStepSafety,
       WrongSuccessorCandidateStepSafety,
       Next, ValidateFinality, CloseWorkerIngress,
       ClearOneWorkerExactOutput, SealAppliedHeightOutputHandoff,
       RejectLateExactOutputEnqueue, BeginExactSuccessorConstruction,
       PresentSameContextForeignOwnerReceipt,
       RejectSameContextForeignOwnerReceipt,
       PresentMismatchedPredecessorContextReceipt,
       PresentMismatchedPredecessorArtifactReceipt,
       RejectMismatchedPredecessorReceipt,
       PresentWrongImmediateSuccessor, RejectWrongImmediateSuccessor,
       ConsumeReceiptIntoRetainedMergeSidecars,
       PersistNextServiceHighWater,
       FailNextServiceHighWaterPersistence,
       FailLifecycleSnapshotAfterHighWaterPersistence,
       CrashAtHighWaterAheadSnapshot,
       RejectTornHighWaterSnapshotOpen,
       RejectOrdinaryLiveChangedRosterTransition,
       QuiesceChangedRosterResponder, TypedChangedRosterTransition,
       SameRosterRetainedTransportRollover,
       ObserveLateOldWriterCallback, CrashWithRolledBackHighWater,
       ValidationCandidatePending, PredecessorMismatchFailure,
       validationHistoryVars, validationHistoriesExceptTorn,
       validationHistoriesExceptForeignOwner,
       validationHistoriesExceptPredecessorCandidate,
       validationHistoriesExceptWrongSuccessorCandidate,
       TornHighWaterSnapshotMidpoint,
       HighWaterAheadOfLifecycleSnapshot,
       PreparedHighWaterSnapshotMidpoint,
       ExactRetainedMergeSidecars, ExactPredecessorReceipt,
       ExactServiceTransportOwnerPair, ExactSuccessorConstruction,
       ResponderBlocksOrdinaryTransition,
       ForeignOwnerNonce, TransportOwnerNonce, ServiceOwnerNonce,
       ExpectedContext, ExpectedArtifact, ExpectedParent,
       ExpectedSuccessor, InitialGeneration, NextGeneration

THEOREM TypedRolloverNextPreservesSafety ==
  /\ TypedRolloverSafetyInvariant
  /\ Next
  => TypedRolloverSafetyInvariant'
PROOF
  <1>1. CASE ValidateFinality
    BY <1>1, SMTT(45)
       DEF TypedRolloverSafetyInvariant,
           TypedRolloverTypeInvariant, ReceiptLifecycleInvariant,
           FinalSealRejectsLateEnqueueInvariant,
           ReceiptExactOwnerAndPredecessorInvariant,
           RetainedWrapperIdentityInvariant,
           TransitionAuthorityLifecycleInvariant,
           OrdinaryChangedRosterTransitionInvariant,
           LiveChangedRosterNeedsTypedReceiptInvariant,
           TypedTransitionAtomicClearInvariant,
           SameRosterTransportPreservationInvariant,
           SameRosterRetryPreservationInvariant,
           RetryChunkHasLiveOwnerInvariant,
           DurableHighWaterFailClosedInvariant,
           HighWaterAheadSnapshotInvariant,
           TornMidpointOpenRejectionInvariant,
           TornHighWaterHistoryOriginInvariant,
           ForeignOwnerCandidateRejectionInvariant,
           PredecessorMismatchCandidateRejectionInvariant,
           WrongSuccessorCandidateRejectionInvariant,
           FailureLatchInvariant,
           PredecessorMismatchFailure, ValidationCandidatePending,
           validationHistoryVars, validationHistoriesExceptTorn,
           validationHistoriesExceptForeignOwner,
           validationHistoriesExceptPredecessorCandidate,
           validationHistoriesExceptWrongSuccessorCandidate,
           LateOldCallbackIsolationInvariant,
           HighWaterAheadOfLifecycleSnapshot,
           PreparedHighWaterSnapshotMidpoint,
           TornHighWaterSnapshotMidpoint, ValidateFinality,
           ExactPredecessorReceipt, ExactServiceTransportOwnerPair,
           ExactSuccessorConstruction, RosterRelations, ReceiptStages,
           TransitionAuthorities, FailureReasons, OwnerNonces, PredecessorMismatchKinds, Contexts,
           Artifacts, Parents, Successors, NoIdentity,
           InitialWorkerOutstanding, InitialGeneration, NextGeneration
  <1>2. CASE CloseWorkerIngress
    BY <1>2, SMTT(45)
       DEF TypedRolloverSafetyInvariant,
           TypedRolloverTypeInvariant, ReceiptLifecycleInvariant,
           FinalSealRejectsLateEnqueueInvariant,
           ReceiptExactOwnerAndPredecessorInvariant,
           RetainedWrapperIdentityInvariant,
           TransitionAuthorityLifecycleInvariant,
           OrdinaryChangedRosterTransitionInvariant,
           LiveChangedRosterNeedsTypedReceiptInvariant,
           TypedTransitionAtomicClearInvariant,
           SameRosterTransportPreservationInvariant,
           SameRosterRetryPreservationInvariant,
           RetryChunkHasLiveOwnerInvariant,
           DurableHighWaterFailClosedInvariant,
           HighWaterAheadSnapshotInvariant,
           TornMidpointOpenRejectionInvariant,
           TornHighWaterHistoryOriginInvariant,
           ForeignOwnerCandidateRejectionInvariant,
           PredecessorMismatchCandidateRejectionInvariant,
           WrongSuccessorCandidateRejectionInvariant,
           FailureLatchInvariant,
           PredecessorMismatchFailure, ValidationCandidatePending,
           validationHistoryVars, validationHistoriesExceptTorn,
           validationHistoriesExceptForeignOwner,
           validationHistoriesExceptPredecessorCandidate,
           validationHistoriesExceptWrongSuccessorCandidate,
           LateOldCallbackIsolationInvariant,
           HighWaterAheadOfLifecycleSnapshot,
           PreparedHighWaterSnapshotMidpoint,
           TornHighWaterSnapshotMidpoint, CloseWorkerIngress,
           ExactPredecessorReceipt, ExactServiceTransportOwnerPair,
           ExactSuccessorConstruction, RosterRelations, ReceiptStages,
           TransitionAuthorities, FailureReasons, OwnerNonces, PredecessorMismatchKinds, Contexts,
           Artifacts, Parents, Successors, NoIdentity,
           InitialWorkerOutstanding, InitialGeneration, NextGeneration
  <1>3. CASE ClearOneWorkerExactOutput
    BY <1>3, SMTT(45)
       DEF TypedRolloverSafetyInvariant,
           TypedRolloverTypeInvariant, ReceiptLifecycleInvariant,
           FinalSealRejectsLateEnqueueInvariant,
           ReceiptExactOwnerAndPredecessorInvariant,
           RetainedWrapperIdentityInvariant,
           TransitionAuthorityLifecycleInvariant,
           OrdinaryChangedRosterTransitionInvariant,
           LiveChangedRosterNeedsTypedReceiptInvariant,
           TypedTransitionAtomicClearInvariant,
           SameRosterTransportPreservationInvariant,
           SameRosterRetryPreservationInvariant,
           RetryChunkHasLiveOwnerInvariant,
           DurableHighWaterFailClosedInvariant,
           HighWaterAheadSnapshotInvariant,
           TornMidpointOpenRejectionInvariant,
           TornHighWaterHistoryOriginInvariant,
           ForeignOwnerCandidateRejectionInvariant,
           PredecessorMismatchCandidateRejectionInvariant,
           WrongSuccessorCandidateRejectionInvariant,
           FailureLatchInvariant,
           PredecessorMismatchFailure, ValidationCandidatePending,
           validationHistoryVars, validationHistoriesExceptTorn,
           validationHistoriesExceptForeignOwner,
           validationHistoriesExceptPredecessorCandidate,
           validationHistoriesExceptWrongSuccessorCandidate,
           LateOldCallbackIsolationInvariant,
           HighWaterAheadOfLifecycleSnapshot,
           PreparedHighWaterSnapshotMidpoint,
           TornHighWaterSnapshotMidpoint, ClearOneWorkerExactOutput,
           ExactPredecessorReceipt, ExactServiceTransportOwnerPair,
           ExactSuccessorConstruction, RosterRelations, ReceiptStages,
           TransitionAuthorities, FailureReasons, OwnerNonces, PredecessorMismatchKinds, Contexts,
           Artifacts, Parents, Successors, NoIdentity,
           InitialWorkerOutstanding, InitialGeneration, NextGeneration
  <1>4. CASE SealAppliedHeightOutputHandoff
    BY <1>4, SMTT(45)
       DEF TypedRolloverSafetyInvariant,
           TypedRolloverTypeInvariant, ReceiptLifecycleInvariant,
           FinalSealRejectsLateEnqueueInvariant,
           ReceiptExactOwnerAndPredecessorInvariant,
           RetainedWrapperIdentityInvariant,
           TransitionAuthorityLifecycleInvariant,
           OrdinaryChangedRosterTransitionInvariant,
           LiveChangedRosterNeedsTypedReceiptInvariant,
           TypedTransitionAtomicClearInvariant,
           SameRosterTransportPreservationInvariant,
           SameRosterRetryPreservationInvariant,
           RetryChunkHasLiveOwnerInvariant,
           DurableHighWaterFailClosedInvariant,
           HighWaterAheadSnapshotInvariant,
           TornMidpointOpenRejectionInvariant,
           TornHighWaterHistoryOriginInvariant,
           ForeignOwnerCandidateRejectionInvariant,
           PredecessorMismatchCandidateRejectionInvariant,
           WrongSuccessorCandidateRejectionInvariant,
           FailureLatchInvariant,
           PredecessorMismatchFailure, ValidationCandidatePending,
           validationHistoryVars, validationHistoriesExceptTorn,
           validationHistoriesExceptForeignOwner,
           validationHistoriesExceptPredecessorCandidate,
           validationHistoriesExceptWrongSuccessorCandidate,
           LateOldCallbackIsolationInvariant,
           HighWaterAheadOfLifecycleSnapshot,
           PreparedHighWaterSnapshotMidpoint,
           TornHighWaterSnapshotMidpoint,
           SealAppliedHeightOutputHandoff,
           ExactPredecessorReceipt, ExactServiceTransportOwnerPair,
           ExactSuccessorConstruction, RosterRelations, ReceiptStages,
           TransitionAuthorities, FailureReasons, OwnerNonces, PredecessorMismatchKinds, Contexts,
           Artifacts, Parents, Successors, NoIdentity, ServiceOwnerNonce,
           TransportOwnerNonce, ExpectedContext, ExpectedArtifact,
           InitialWorkerOutstanding, InitialGeneration, NextGeneration
  <1>5. CASE RejectLateExactOutputEnqueue
    BY <1>5, SMTT(45)
       DEF TypedRolloverSafetyInvariant,
           TypedRolloverTypeInvariant, ReceiptLifecycleInvariant,
           FinalSealRejectsLateEnqueueInvariant,
           ReceiptExactOwnerAndPredecessorInvariant,
           RetainedWrapperIdentityInvariant,
           TransitionAuthorityLifecycleInvariant,
           OrdinaryChangedRosterTransitionInvariant,
           LiveChangedRosterNeedsTypedReceiptInvariant,
           TypedTransitionAtomicClearInvariant,
           SameRosterTransportPreservationInvariant,
           SameRosterRetryPreservationInvariant,
           RetryChunkHasLiveOwnerInvariant,
           DurableHighWaterFailClosedInvariant,
           HighWaterAheadSnapshotInvariant,
           TornMidpointOpenRejectionInvariant,
           TornHighWaterHistoryOriginInvariant,
           ForeignOwnerCandidateRejectionInvariant,
           PredecessorMismatchCandidateRejectionInvariant,
           WrongSuccessorCandidateRejectionInvariant,
           FailureLatchInvariant,
           PredecessorMismatchFailure, ValidationCandidatePending,
           validationHistoryVars, validationHistoriesExceptTorn,
           validationHistoriesExceptForeignOwner,
           validationHistoriesExceptPredecessorCandidate,
           validationHistoriesExceptWrongSuccessorCandidate,
           LateOldCallbackIsolationInvariant,
           HighWaterAheadOfLifecycleSnapshot,
           PreparedHighWaterSnapshotMidpoint,
           TornHighWaterSnapshotMidpoint,
           RejectLateExactOutputEnqueue,
           ExactPredecessorReceipt, ExactServiceTransportOwnerPair,
           ExactSuccessorConstruction, RosterRelations, ReceiptStages,
           TransitionAuthorities, FailureReasons, OwnerNonces, PredecessorMismatchKinds, Contexts,
           Artifacts, Parents, Successors, NoIdentity,
           InitialWorkerOutstanding, InitialGeneration, NextGeneration
  <1>6. CASE BeginExactSuccessorConstruction
    BY <1>6, SMTT(45)
       DEF TypedRolloverSafetyInvariant,
           TypedRolloverTypeInvariant, ReceiptLifecycleInvariant,
           FinalSealRejectsLateEnqueueInvariant,
           ReceiptExactOwnerAndPredecessorInvariant,
           RetainedWrapperIdentityInvariant,
           TransitionAuthorityLifecycleInvariant,
           OrdinaryChangedRosterTransitionInvariant,
           LiveChangedRosterNeedsTypedReceiptInvariant,
           TypedTransitionAtomicClearInvariant,
           SameRosterTransportPreservationInvariant,
           SameRosterRetryPreservationInvariant,
           RetryChunkHasLiveOwnerInvariant,
           DurableHighWaterFailClosedInvariant,
           HighWaterAheadSnapshotInvariant,
           TornMidpointOpenRejectionInvariant,
           TornHighWaterHistoryOriginInvariant,
           ForeignOwnerCandidateRejectionInvariant,
           PredecessorMismatchCandidateRejectionInvariant,
           WrongSuccessorCandidateRejectionInvariant,
           FailureLatchInvariant,
           PredecessorMismatchFailure, ValidationCandidatePending,
           validationHistoryVars, validationHistoriesExceptTorn,
           validationHistoriesExceptForeignOwner,
           validationHistoriesExceptPredecessorCandidate,
           validationHistoriesExceptWrongSuccessorCandidate,
           LateOldCallbackIsolationInvariant,
           HighWaterAheadOfLifecycleSnapshot,
           PreparedHighWaterSnapshotMidpoint,
           TornHighWaterSnapshotMidpoint,
           BeginExactSuccessorConstruction,
           ExactPredecessorReceipt, ExactServiceTransportOwnerPair,
           ExactSuccessorConstruction, RosterRelations, ReceiptStages,
           TransitionAuthorities, FailureReasons, OwnerNonces, PredecessorMismatchKinds, Contexts,
           Artifacts, Parents, Successors, NoIdentity, ExpectedParent,
           ExpectedSuccessor, InitialWorkerOutstanding,
           InitialGeneration, NextGeneration
  <1>7. CASE RejectSameContextForeignOwnerReceipt
    BY <1>7, SMTT(45)
       DEF TypedRolloverSafetyInvariant,
           TypedRolloverTypeInvariant, ReceiptLifecycleInvariant,
           FinalSealRejectsLateEnqueueInvariant,
           ReceiptExactOwnerAndPredecessorInvariant,
           RetainedWrapperIdentityInvariant,
           TransitionAuthorityLifecycleInvariant,
           OrdinaryChangedRosterTransitionInvariant,
           LiveChangedRosterNeedsTypedReceiptInvariant,
           TypedTransitionAtomicClearInvariant,
           SameRosterTransportPreservationInvariant,
           SameRosterRetryPreservationInvariant,
           RetryChunkHasLiveOwnerInvariant,
           DurableHighWaterFailClosedInvariant,
           HighWaterAheadSnapshotInvariant,
           TornMidpointOpenRejectionInvariant,
           TornHighWaterHistoryOriginInvariant,
           ForeignOwnerCandidateRejectionInvariant,
           PredecessorMismatchCandidateRejectionInvariant,
           WrongSuccessorCandidateRejectionInvariant,
           FailureLatchInvariant,
           PredecessorMismatchFailure, ValidationCandidatePending,
           validationHistoryVars, validationHistoriesExceptTorn,
           validationHistoriesExceptForeignOwner,
           validationHistoriesExceptPredecessorCandidate,
           validationHistoriesExceptWrongSuccessorCandidate,
           LateOldCallbackIsolationInvariant,
           HighWaterAheadOfLifecycleSnapshot,
           PreparedHighWaterSnapshotMidpoint,
           TornHighWaterSnapshotMidpoint,
           RejectSameContextForeignOwnerReceipt,
           ExactPredecessorReceipt, ExactServiceTransportOwnerPair,
           ExactSuccessorConstruction, RosterRelations, ReceiptStages,
           TransitionAuthorities, FailureReasons, OwnerNonces, PredecessorMismatchKinds, Contexts,
           Artifacts, Parents, Successors, NoIdentity,
           InitialWorkerOutstanding, InitialGeneration, NextGeneration
  <1>8. CASE ConsumeReceiptIntoRetainedMergeSidecars
    BY <1>8, SMTT(45)
       DEF TypedRolloverSafetyInvariant,
           TypedRolloverTypeInvariant, ReceiptLifecycleInvariant,
           FinalSealRejectsLateEnqueueInvariant,
           ReceiptExactOwnerAndPredecessorInvariant,
           RetainedWrapperIdentityInvariant,
           TransitionAuthorityLifecycleInvariant,
           OrdinaryChangedRosterTransitionInvariant,
           LiveChangedRosterNeedsTypedReceiptInvariant,
           TypedTransitionAtomicClearInvariant,
           SameRosterTransportPreservationInvariant,
           SameRosterRetryPreservationInvariant,
           RetryChunkHasLiveOwnerInvariant,
           DurableHighWaterFailClosedInvariant,
           HighWaterAheadSnapshotInvariant,
           TornMidpointOpenRejectionInvariant,
           TornHighWaterHistoryOriginInvariant,
           ForeignOwnerCandidateRejectionInvariant,
           PredecessorMismatchCandidateRejectionInvariant,
           WrongSuccessorCandidateRejectionInvariant,
           FailureLatchInvariant,
           PredecessorMismatchFailure, ValidationCandidatePending,
           validationHistoryVars, validationHistoriesExceptTorn,
           validationHistoriesExceptForeignOwner,
           validationHistoriesExceptPredecessorCandidate,
           validationHistoriesExceptWrongSuccessorCandidate,
           LateOldCallbackIsolationInvariant,
           HighWaterAheadOfLifecycleSnapshot,
           PreparedHighWaterSnapshotMidpoint,
           TornHighWaterSnapshotMidpoint,
           ConsumeReceiptIntoRetainedMergeSidecars,
           ExactPredecessorReceipt, ExactServiceTransportOwnerPair,
           ExactSuccessorConstruction, RosterRelations, ReceiptStages,
           TransitionAuthorities, FailureReasons, OwnerNonces, PredecessorMismatchKinds, Contexts,
           Artifacts, Parents, Successors, NoIdentity, ExpectedSuccessor,
           InitialWorkerOutstanding, InitialGeneration, NextGeneration
  <1>9. CASE PersistNextServiceHighWater
    BY <1>9, SMTT(45)
       DEF TypedRolloverSafetyInvariant,
           TypedRolloverTypeInvariant, ReceiptLifecycleInvariant,
           FinalSealRejectsLateEnqueueInvariant,
           ReceiptExactOwnerAndPredecessorInvariant,
           RetainedWrapperIdentityInvariant,
           TransitionAuthorityLifecycleInvariant,
           OrdinaryChangedRosterTransitionInvariant,
           LiveChangedRosterNeedsTypedReceiptInvariant,
           TypedTransitionAtomicClearInvariant,
           SameRosterTransportPreservationInvariant,
           SameRosterRetryPreservationInvariant,
           RetryChunkHasLiveOwnerInvariant,
           DurableHighWaterFailClosedInvariant,
           HighWaterAheadSnapshotInvariant,
           TornMidpointOpenRejectionInvariant,
           TornHighWaterHistoryOriginInvariant,
           ForeignOwnerCandidateRejectionInvariant,
           PredecessorMismatchCandidateRejectionInvariant,
           WrongSuccessorCandidateRejectionInvariant,
           FailureLatchInvariant,
           PredecessorMismatchFailure, ValidationCandidatePending,
           validationHistoryVars, validationHistoriesExceptTorn,
           validationHistoriesExceptForeignOwner,
           validationHistoriesExceptPredecessorCandidate,
           validationHistoriesExceptWrongSuccessorCandidate,
           LateOldCallbackIsolationInvariant,
           HighWaterAheadOfLifecycleSnapshot,
           PreparedHighWaterSnapshotMidpoint,
           TornHighWaterSnapshotMidpoint,
           PersistNextServiceHighWater, ExactRetainedMergeSidecars,
           ExactPredecessorReceipt, ExactServiceTransportOwnerPair,
           ExactSuccessorConstruction, RosterRelations, ReceiptStages,
           TransitionAuthorities, FailureReasons, OwnerNonces, PredecessorMismatchKinds, Contexts,
           Artifacts, Parents, Successors, NoIdentity,
           InitialWorkerOutstanding, InitialGeneration, NextGeneration
  <1>10. CASE FailNextServiceHighWaterPersistence
    BY <1>10, SMTT(45)
       DEF TypedRolloverSafetyInvariant,
           TypedRolloverTypeInvariant, ReceiptLifecycleInvariant,
           FinalSealRejectsLateEnqueueInvariant,
           ReceiptExactOwnerAndPredecessorInvariant,
           RetainedWrapperIdentityInvariant,
           TransitionAuthorityLifecycleInvariant,
           OrdinaryChangedRosterTransitionInvariant,
           LiveChangedRosterNeedsTypedReceiptInvariant,
           TypedTransitionAtomicClearInvariant,
           SameRosterTransportPreservationInvariant,
           SameRosterRetryPreservationInvariant,
           RetryChunkHasLiveOwnerInvariant,
           DurableHighWaterFailClosedInvariant,
           HighWaterAheadSnapshotInvariant,
           TornMidpointOpenRejectionInvariant,
           TornHighWaterHistoryOriginInvariant,
           ForeignOwnerCandidateRejectionInvariant,
           PredecessorMismatchCandidateRejectionInvariant,
           WrongSuccessorCandidateRejectionInvariant,
           FailureLatchInvariant,
           PredecessorMismatchFailure, ValidationCandidatePending,
           validationHistoryVars, validationHistoriesExceptTorn,
           validationHistoriesExceptForeignOwner,
           validationHistoriesExceptPredecessorCandidate,
           validationHistoriesExceptWrongSuccessorCandidate,
           LateOldCallbackIsolationInvariant,
           HighWaterAheadOfLifecycleSnapshot,
           PreparedHighWaterSnapshotMidpoint,
           TornHighWaterSnapshotMidpoint,
           FailNextServiceHighWaterPersistence,
           ExactRetainedMergeSidecars, ExactPredecessorReceipt,
           ExactServiceTransportOwnerPair, ExactSuccessorConstruction,
           RosterRelations, ReceiptStages, TransitionAuthorities,
           FailureReasons, OwnerNonces, PredecessorMismatchKinds, Contexts, Artifacts, Parents,
           Successors, NoIdentity, InitialWorkerOutstanding,
           InitialGeneration, NextGeneration
  <1>11. CASE RejectOrdinaryLiveChangedRosterTransition
    BY <1>11, SMTT(45)
       DEF TypedRolloverSafetyInvariant,
           TypedRolloverTypeInvariant, ReceiptLifecycleInvariant,
           FinalSealRejectsLateEnqueueInvariant,
           ReceiptExactOwnerAndPredecessorInvariant,
           RetainedWrapperIdentityInvariant,
           TransitionAuthorityLifecycleInvariant,
           OrdinaryChangedRosterTransitionInvariant,
           LiveChangedRosterNeedsTypedReceiptInvariant,
           TypedTransitionAtomicClearInvariant,
           SameRosterTransportPreservationInvariant,
           SameRosterRetryPreservationInvariant,
           RetryChunkHasLiveOwnerInvariant,
           DurableHighWaterFailClosedInvariant,
           HighWaterAheadSnapshotInvariant,
           TornMidpointOpenRejectionInvariant,
           TornHighWaterHistoryOriginInvariant,
           ForeignOwnerCandidateRejectionInvariant,
           PredecessorMismatchCandidateRejectionInvariant,
           WrongSuccessorCandidateRejectionInvariant,
           FailureLatchInvariant,
           PredecessorMismatchFailure, ValidationCandidatePending,
           validationHistoryVars, validationHistoriesExceptTorn,
           validationHistoriesExceptForeignOwner,
           validationHistoriesExceptPredecessorCandidate,
           validationHistoriesExceptWrongSuccessorCandidate,
           LateOldCallbackIsolationInvariant,
           HighWaterAheadOfLifecycleSnapshot,
           PreparedHighWaterSnapshotMidpoint,
           TornHighWaterSnapshotMidpoint,
           RejectOrdinaryLiveChangedRosterTransition,
           ExactPredecessorReceipt, ExactServiceTransportOwnerPair,
           ExactSuccessorConstruction, RosterRelations, ReceiptStages,
           TransitionAuthorities, FailureReasons, OwnerNonces, PredecessorMismatchKinds, Contexts,
           Artifacts, Parents, Successors, NoIdentity,
           InitialWorkerOutstanding, InitialGeneration, NextGeneration
  <1>12. CASE QuiesceChangedRosterResponder
    BY <1>12, SMTT(45)
       DEF TypedRolloverSafetyInvariant,
           TypedRolloverTypeInvariant, ReceiptLifecycleInvariant,
           FinalSealRejectsLateEnqueueInvariant,
           ReceiptExactOwnerAndPredecessorInvariant,
           RetainedWrapperIdentityInvariant,
           TransitionAuthorityLifecycleInvariant,
           OrdinaryChangedRosterTransitionInvariant,
           LiveChangedRosterNeedsTypedReceiptInvariant,
           TypedTransitionAtomicClearInvariant,
           SameRosterTransportPreservationInvariant,
           SameRosterRetryPreservationInvariant,
           RetryChunkHasLiveOwnerInvariant,
           DurableHighWaterFailClosedInvariant,
           HighWaterAheadSnapshotInvariant,
           TornMidpointOpenRejectionInvariant,
           TornHighWaterHistoryOriginInvariant,
           ForeignOwnerCandidateRejectionInvariant,
           PredecessorMismatchCandidateRejectionInvariant,
           WrongSuccessorCandidateRejectionInvariant,
           FailureLatchInvariant,
           PredecessorMismatchFailure, ValidationCandidatePending,
           validationHistoryVars, validationHistoriesExceptTorn,
           validationHistoriesExceptForeignOwner,
           validationHistoriesExceptPredecessorCandidate,
           validationHistoriesExceptWrongSuccessorCandidate,
           LateOldCallbackIsolationInvariant,
           HighWaterAheadOfLifecycleSnapshot,
           PreparedHighWaterSnapshotMidpoint,
           TornHighWaterSnapshotMidpoint,
           QuiesceChangedRosterResponder,
           ResponderBlocksOrdinaryTransition,
           ExactPredecessorReceipt, ExactServiceTransportOwnerPair,
           ExactSuccessorConstruction, RosterRelations, ReceiptStages,
           TransitionAuthorities, FailureReasons, OwnerNonces, PredecessorMismatchKinds, Contexts,
           Artifacts, Parents, Successors, NoIdentity,
           InitialWorkerOutstanding, InitialGeneration, NextGeneration
  <1>13. CASE TypedChangedRosterTransition
    BY <1>13, SMTT(45)
       DEF TypedRolloverSafetyInvariant,
           TypedRolloverTypeInvariant, ReceiptLifecycleInvariant,
           FinalSealRejectsLateEnqueueInvariant,
           ReceiptExactOwnerAndPredecessorInvariant,
           RetainedWrapperIdentityInvariant,
           TransitionAuthorityLifecycleInvariant,
           OrdinaryChangedRosterTransitionInvariant,
           LiveChangedRosterNeedsTypedReceiptInvariant,
           TypedTransitionAtomicClearInvariant,
           SameRosterTransportPreservationInvariant,
           SameRosterRetryPreservationInvariant,
           RetryChunkHasLiveOwnerInvariant,
           DurableHighWaterFailClosedInvariant,
           HighWaterAheadSnapshotInvariant,
           TornMidpointOpenRejectionInvariant,
           TornHighWaterHistoryOriginInvariant,
           ForeignOwnerCandidateRejectionInvariant,
           PredecessorMismatchCandidateRejectionInvariant,
           WrongSuccessorCandidateRejectionInvariant,
           FailureLatchInvariant,
           PredecessorMismatchFailure, ValidationCandidatePending,
           validationHistoryVars, validationHistoriesExceptTorn,
           validationHistoriesExceptForeignOwner,
           validationHistoriesExceptPredecessorCandidate,
           validationHistoriesExceptWrongSuccessorCandidate,
           LateOldCallbackIsolationInvariant,
           HighWaterAheadOfLifecycleSnapshot,
           PreparedHighWaterSnapshotMidpoint,
           TornHighWaterSnapshotMidpoint,
           TypedChangedRosterTransition, ExactRetainedMergeSidecars,
           ResponderBlocksOrdinaryTransition,
           ExactPredecessorReceipt, ExactServiceTransportOwnerPair,
           ExactSuccessorConstruction, RosterRelations, ReceiptStages,
           TransitionAuthorities, FailureReasons, OwnerNonces, PredecessorMismatchKinds, Contexts,
           Artifacts, Parents, Successors, NoIdentity,
           InitialWorkerOutstanding, InitialGeneration, NextGeneration
  <1>14. CASE SameRosterRetainedTransportRollover
    BY <1>14, SMTT(45)
       DEF TypedRolloverSafetyInvariant,
           TypedRolloverTypeInvariant, ReceiptLifecycleInvariant,
           FinalSealRejectsLateEnqueueInvariant,
           ReceiptExactOwnerAndPredecessorInvariant,
           RetainedWrapperIdentityInvariant,
           TransitionAuthorityLifecycleInvariant,
           OrdinaryChangedRosterTransitionInvariant,
           LiveChangedRosterNeedsTypedReceiptInvariant,
           TypedTransitionAtomicClearInvariant,
           SameRosterTransportPreservationInvariant,
           SameRosterRetryPreservationInvariant,
           RetryChunkHasLiveOwnerInvariant,
           DurableHighWaterFailClosedInvariant,
           HighWaterAheadSnapshotInvariant,
           TornMidpointOpenRejectionInvariant,
           TornHighWaterHistoryOriginInvariant,
           ForeignOwnerCandidateRejectionInvariant,
           PredecessorMismatchCandidateRejectionInvariant,
           WrongSuccessorCandidateRejectionInvariant,
           FailureLatchInvariant,
           PredecessorMismatchFailure, ValidationCandidatePending,
           validationHistoryVars, validationHistoriesExceptTorn,
           validationHistoriesExceptForeignOwner,
           validationHistoriesExceptPredecessorCandidate,
           validationHistoriesExceptWrongSuccessorCandidate,
           LateOldCallbackIsolationInvariant,
           HighWaterAheadOfLifecycleSnapshot,
           PreparedHighWaterSnapshotMidpoint,
           TornHighWaterSnapshotMidpoint,
           SameRosterRetainedTransportRollover,
           ExactRetainedMergeSidecars, ResponderBlocksOrdinaryTransition,
           ExactPredecessorReceipt, ExactServiceTransportOwnerPair,
           ExactSuccessorConstruction, RosterRelations, ReceiptStages,
           TransitionAuthorities, FailureReasons, OwnerNonces, PredecessorMismatchKinds, Contexts,
           Artifacts, Parents, Successors, NoIdentity,
           InitialWorkerOutstanding, InitialGeneration, NextGeneration
  <1>15. CASE ObserveLateOldWriterCallback
    BY <1>15, SMTT(45)
       DEF TypedRolloverSafetyInvariant,
           TypedRolloverTypeInvariant, ReceiptLifecycleInvariant,
           FinalSealRejectsLateEnqueueInvariant,
           ReceiptExactOwnerAndPredecessorInvariant,
           RetainedWrapperIdentityInvariant,
           TransitionAuthorityLifecycleInvariant,
           OrdinaryChangedRosterTransitionInvariant,
           LiveChangedRosterNeedsTypedReceiptInvariant,
           TypedTransitionAtomicClearInvariant,
           SameRosterTransportPreservationInvariant,
           SameRosterRetryPreservationInvariant,
           RetryChunkHasLiveOwnerInvariant,
           DurableHighWaterFailClosedInvariant,
           HighWaterAheadSnapshotInvariant,
           TornMidpointOpenRejectionInvariant,
           TornHighWaterHistoryOriginInvariant,
           ForeignOwnerCandidateRejectionInvariant,
           PredecessorMismatchCandidateRejectionInvariant,
           WrongSuccessorCandidateRejectionInvariant,
           FailureLatchInvariant,
           PredecessorMismatchFailure, ValidationCandidatePending,
           validationHistoryVars, validationHistoriesExceptTorn,
           validationHistoriesExceptForeignOwner,
           validationHistoriesExceptPredecessorCandidate,
           validationHistoriesExceptWrongSuccessorCandidate,
           LateOldCallbackIsolationInvariant,
           HighWaterAheadOfLifecycleSnapshot,
           PreparedHighWaterSnapshotMidpoint,
           TornHighWaterSnapshotMidpoint,
           ObserveLateOldWriterCallback,
           ExactPredecessorReceipt, ExactServiceTransportOwnerPair,
           ExactSuccessorConstruction, RosterRelations, ReceiptStages,
           TransitionAuthorities, FailureReasons, OwnerNonces, PredecessorMismatchKinds, Contexts,
           Artifacts, Parents, Successors, NoIdentity,
           InitialWorkerOutstanding, InitialGeneration, NextGeneration
  <1>16. CASE CrashWithRolledBackHighWater
    BY <1>16, SMTT(45)
       DEF TypedRolloverSafetyInvariant,
           TypedRolloverTypeInvariant, ReceiptLifecycleInvariant,
           FinalSealRejectsLateEnqueueInvariant,
           ReceiptExactOwnerAndPredecessorInvariant,
           RetainedWrapperIdentityInvariant,
           TransitionAuthorityLifecycleInvariant,
           OrdinaryChangedRosterTransitionInvariant,
           LiveChangedRosterNeedsTypedReceiptInvariant,
           TypedTransitionAtomicClearInvariant,
           SameRosterTransportPreservationInvariant,
           SameRosterRetryPreservationInvariant,
           RetryChunkHasLiveOwnerInvariant,
           DurableHighWaterFailClosedInvariant,
           HighWaterAheadSnapshotInvariant,
           TornMidpointOpenRejectionInvariant,
           TornHighWaterHistoryOriginInvariant,
           ForeignOwnerCandidateRejectionInvariant,
           PredecessorMismatchCandidateRejectionInvariant,
           WrongSuccessorCandidateRejectionInvariant,
           FailureLatchInvariant,
           PredecessorMismatchFailure, ValidationCandidatePending,
           validationHistoryVars, validationHistoriesExceptTorn,
           validationHistoriesExceptForeignOwner,
           validationHistoriesExceptPredecessorCandidate,
           validationHistoriesExceptWrongSuccessorCandidate,
           LateOldCallbackIsolationInvariant,
           HighWaterAheadOfLifecycleSnapshot,
           PreparedHighWaterSnapshotMidpoint,
           TornHighWaterSnapshotMidpoint,
           CrashWithRolledBackHighWater,
           ExactPredecessorReceipt, ExactServiceTransportOwnerPair,
           ExactSuccessorConstruction, RosterRelations, ReceiptStages,
           TransitionAuthorities, FailureReasons, OwnerNonces, PredecessorMismatchKinds, Contexts,
           Artifacts, Parents, Successors, NoIdentity,
           InitialWorkerOutstanding, InitialGeneration, NextGeneration
  <1>17. CASE PresentSameContextForeignOwnerReceipt
    BY <1>17, SMTT(45)
       DEF TypedRolloverSafetyInvariant,
           TypedRolloverTypeInvariant, ReceiptLifecycleInvariant,
           FinalSealRejectsLateEnqueueInvariant,
           ReceiptExactOwnerAndPredecessorInvariant,
           RetainedWrapperIdentityInvariant,
           TransitionAuthorityLifecycleInvariant,
           OrdinaryChangedRosterTransitionInvariant,
           LiveChangedRosterNeedsTypedReceiptInvariant,
           TypedTransitionAtomicClearInvariant,
           SameRosterTransportPreservationInvariant,
           SameRosterRetryPreservationInvariant,
           RetryChunkHasLiveOwnerInvariant,
           DurableHighWaterFailClosedInvariant,
           HighWaterAheadSnapshotInvariant,
           TornMidpointOpenRejectionInvariant,
           TornHighWaterHistoryOriginInvariant,
           ForeignOwnerCandidateRejectionInvariant,
           PredecessorMismatchCandidateRejectionInvariant,
           WrongSuccessorCandidateRejectionInvariant,
           FailureLatchInvariant,
           PredecessorMismatchFailure, ValidationCandidatePending,
           validationHistoryVars, validationHistoriesExceptTorn,
           validationHistoriesExceptForeignOwner,
           validationHistoriesExceptPredecessorCandidate,
           validationHistoriesExceptWrongSuccessorCandidate,
           LateOldCallbackIsolationInvariant,
           HighWaterAheadOfLifecycleSnapshot,
           PreparedHighWaterSnapshotMidpoint,
           TornHighWaterSnapshotMidpoint,
           PresentSameContextForeignOwnerReceipt,
           ExactPredecessorReceipt, ExactServiceTransportOwnerPair,
           ExactSuccessorConstruction,
           RosterRelations, ReceiptStages, TransitionAuthorities,
           FailureReasons, OwnerNonces, PredecessorMismatchKinds, Contexts, Artifacts, Parents,
           Successors, NoIdentity, ExpectedParent, ExpectedSuccessor,
           InitialWorkerOutstanding, InitialGeneration, NextGeneration
  <1>18. CASE FailLifecycleSnapshotAfterHighWaterPersistence
    BY <1>18, SMTT(45)
       DEF TypedRolloverSafetyInvariant,
           TypedRolloverTypeInvariant, ReceiptLifecycleInvariant,
           FinalSealRejectsLateEnqueueInvariant,
           ReceiptExactOwnerAndPredecessorInvariant,
           RetainedWrapperIdentityInvariant,
           TransitionAuthorityLifecycleInvariant,
           OrdinaryChangedRosterTransitionInvariant,
           LiveChangedRosterNeedsTypedReceiptInvariant,
           TypedTransitionAtomicClearInvariant,
           SameRosterTransportPreservationInvariant,
           SameRosterRetryPreservationInvariant,
           RetryChunkHasLiveOwnerInvariant,
           DurableHighWaterFailClosedInvariant,
           HighWaterAheadSnapshotInvariant,
           TornMidpointOpenRejectionInvariant,
           TornHighWaterHistoryOriginInvariant,
           ForeignOwnerCandidateRejectionInvariant,
           PredecessorMismatchCandidateRejectionInvariant,
           WrongSuccessorCandidateRejectionInvariant,
           FailureLatchInvariant,
           PredecessorMismatchFailure, ValidationCandidatePending,
           validationHistoryVars, validationHistoriesExceptTorn,
           validationHistoriesExceptForeignOwner,
           validationHistoriesExceptPredecessorCandidate,
           validationHistoriesExceptWrongSuccessorCandidate,
           LateOldCallbackIsolationInvariant,
           HighWaterAheadOfLifecycleSnapshot,
           PreparedHighWaterSnapshotMidpoint,
           TornHighWaterSnapshotMidpoint,
           FailLifecycleSnapshotAfterHighWaterPersistence,
           ExactRetainedMergeSidecars, ExactPredecessorReceipt,
           ExactServiceTransportOwnerPair, ExactSuccessorConstruction,
           RosterRelations, ReceiptStages, TransitionAuthorities,
           FailureReasons, OwnerNonces, PredecessorMismatchKinds, Contexts, Artifacts, Parents,
           Successors, NoIdentity, InitialWorkerOutstanding,
           InitialGeneration, NextGeneration
  <1>19. CASE CrashAtHighWaterAheadSnapshot
    BY <1>19, SMTT(45)
       DEF TypedRolloverSafetyInvariant,
           TypedRolloverTypeInvariant, ReceiptLifecycleInvariant,
           FinalSealRejectsLateEnqueueInvariant,
           ReceiptExactOwnerAndPredecessorInvariant,
           RetainedWrapperIdentityInvariant,
           TransitionAuthorityLifecycleInvariant,
           OrdinaryChangedRosterTransitionInvariant,
           LiveChangedRosterNeedsTypedReceiptInvariant,
           TypedTransitionAtomicClearInvariant,
           SameRosterTransportPreservationInvariant,
           SameRosterRetryPreservationInvariant,
           RetryChunkHasLiveOwnerInvariant,
           DurableHighWaterFailClosedInvariant,
           HighWaterAheadSnapshotInvariant,
           TornMidpointOpenRejectionInvariant,
           TornHighWaterHistoryOriginInvariant,
           ForeignOwnerCandidateRejectionInvariant,
           PredecessorMismatchCandidateRejectionInvariant,
           WrongSuccessorCandidateRejectionInvariant,
           FailureLatchInvariant,
           PredecessorMismatchFailure, ValidationCandidatePending,
           validationHistoryVars, validationHistoriesExceptTorn,
           validationHistoriesExceptForeignOwner,
           validationHistoriesExceptPredecessorCandidate,
           validationHistoriesExceptWrongSuccessorCandidate,
           LateOldCallbackIsolationInvariant,
           HighWaterAheadOfLifecycleSnapshot,
           PreparedHighWaterSnapshotMidpoint,
           TornHighWaterSnapshotMidpoint,
           CrashAtHighWaterAheadSnapshot, ExactRetainedMergeSidecars,
           ExactPredecessorReceipt, ExactServiceTransportOwnerPair,
           ExactSuccessorConstruction,
           RosterRelations, ReceiptStages, TransitionAuthorities,
           FailureReasons, OwnerNonces, PredecessorMismatchKinds, Contexts, Artifacts, Parents,
           Successors, NoIdentity, InitialWorkerOutstanding,
           InitialGeneration, NextGeneration
  <1>20. CASE RejectTornHighWaterSnapshotOpen
    BY <1>20, SMTT(45)
       DEF TypedRolloverSafetyInvariant,
           TypedRolloverTypeInvariant, ReceiptLifecycleInvariant,
           FinalSealRejectsLateEnqueueInvariant,
           ReceiptExactOwnerAndPredecessorInvariant,
           RetainedWrapperIdentityInvariant,
           TransitionAuthorityLifecycleInvariant,
           OrdinaryChangedRosterTransitionInvariant,
           LiveChangedRosterNeedsTypedReceiptInvariant,
           TypedTransitionAtomicClearInvariant,
           SameRosterTransportPreservationInvariant,
           SameRosterRetryPreservationInvariant,
           RetryChunkHasLiveOwnerInvariant,
           DurableHighWaterFailClosedInvariant,
           HighWaterAheadSnapshotInvariant,
           TornMidpointOpenRejectionInvariant,
           TornHighWaterHistoryOriginInvariant,
           ForeignOwnerCandidateRejectionInvariant,
           PredecessorMismatchCandidateRejectionInvariant,
           WrongSuccessorCandidateRejectionInvariant,
           FailureLatchInvariant,
           PredecessorMismatchFailure, ValidationCandidatePending,
           validationHistoryVars, validationHistoriesExceptTorn,
           validationHistoriesExceptForeignOwner,
           validationHistoriesExceptPredecessorCandidate,
           validationHistoriesExceptWrongSuccessorCandidate,
           LateOldCallbackIsolationInvariant,
           HighWaterAheadOfLifecycleSnapshot,
           PreparedHighWaterSnapshotMidpoint,
           TornHighWaterSnapshotMidpoint,
           RejectTornHighWaterSnapshotOpen,
           ExactPredecessorReceipt, ExactServiceTransportOwnerPair,
           ExactSuccessorConstruction,
           RosterRelations, ReceiptStages, TransitionAuthorities,
           FailureReasons, OwnerNonces, PredecessorMismatchKinds, Contexts, Artifacts, Parents,
           Successors, NoIdentity, InitialWorkerOutstanding,
           InitialGeneration, NextGeneration
  <1>21. CASE PresentMismatchedPredecessorContextReceipt
    BY <1>21, SMTT(45)
       DEF TypedRolloverSafetyInvariant,
           TypedRolloverTypeInvariant, ReceiptLifecycleInvariant,
           FinalSealRejectsLateEnqueueInvariant,
           ReceiptExactOwnerAndPredecessorInvariant,
           RetainedWrapperIdentityInvariant,
           TransitionAuthorityLifecycleInvariant,
           OrdinaryChangedRosterTransitionInvariant,
           LiveChangedRosterNeedsTypedReceiptInvariant,
           TypedTransitionAtomicClearInvariant,
           SameRosterTransportPreservationInvariant,
           SameRosterRetryPreservationInvariant,
           RetryChunkHasLiveOwnerInvariant,
           DurableHighWaterFailClosedInvariant,
           HighWaterAheadSnapshotInvariant,
           TornMidpointOpenRejectionInvariant,
           TornHighWaterHistoryOriginInvariant,
           ForeignOwnerCandidateRejectionInvariant,
           PredecessorMismatchCandidateRejectionInvariant,
           WrongSuccessorCandidateRejectionInvariant,
           FailureLatchInvariant, PredecessorMismatchFailure,
           ValidationCandidatePending, validationHistoryVars,
           validationHistoriesExceptTorn,
           validationHistoriesExceptForeignOwner,
           validationHistoriesExceptPredecessorCandidate,
           validationHistoriesExceptWrongSuccessorCandidate,
           LateOldCallbackIsolationInvariant,
           HighWaterAheadOfLifecycleSnapshot,
           PreparedHighWaterSnapshotMidpoint,
           TornHighWaterSnapshotMidpoint,
           PresentMismatchedPredecessorContextReceipt,
           ExactPredecessorReceipt, ExactServiceTransportOwnerPair,
           ExactSuccessorConstruction, RosterRelations, ReceiptStages,
           TransitionAuthorities, FailureReasons, OwnerNonces,
           PredecessorMismatchKinds, Contexts, Artifacts, Parents,
           Successors, NoIdentity, ExpectedParent, ExpectedSuccessor,
           InitialWorkerOutstanding, InitialGeneration, NextGeneration
  <1>22. CASE PresentMismatchedPredecessorArtifactReceipt
    BY <1>22, SMTT(45)
       DEF TypedRolloverSafetyInvariant,
           TypedRolloverTypeInvariant, ReceiptLifecycleInvariant,
           FinalSealRejectsLateEnqueueInvariant,
           ReceiptExactOwnerAndPredecessorInvariant,
           RetainedWrapperIdentityInvariant,
           TransitionAuthorityLifecycleInvariant,
           OrdinaryChangedRosterTransitionInvariant,
           LiveChangedRosterNeedsTypedReceiptInvariant,
           TypedTransitionAtomicClearInvariant,
           SameRosterTransportPreservationInvariant,
           SameRosterRetryPreservationInvariant,
           RetryChunkHasLiveOwnerInvariant,
           DurableHighWaterFailClosedInvariant,
           HighWaterAheadSnapshotInvariant,
           TornMidpointOpenRejectionInvariant,
           TornHighWaterHistoryOriginInvariant,
           ForeignOwnerCandidateRejectionInvariant,
           PredecessorMismatchCandidateRejectionInvariant,
           WrongSuccessorCandidateRejectionInvariant,
           FailureLatchInvariant, PredecessorMismatchFailure,
           ValidationCandidatePending, validationHistoryVars,
           validationHistoriesExceptTorn,
           validationHistoriesExceptForeignOwner,
           validationHistoriesExceptPredecessorCandidate,
           validationHistoriesExceptWrongSuccessorCandidate,
           LateOldCallbackIsolationInvariant,
           HighWaterAheadOfLifecycleSnapshot,
           PreparedHighWaterSnapshotMidpoint,
           TornHighWaterSnapshotMidpoint,
           PresentMismatchedPredecessorArtifactReceipt,
           ExactPredecessorReceipt, ExactServiceTransportOwnerPair,
           ExactSuccessorConstruction, RosterRelations, ReceiptStages,
           TransitionAuthorities, FailureReasons, OwnerNonces,
           PredecessorMismatchKinds, Contexts, Artifacts, Parents,
           Successors, NoIdentity, ExpectedParent, ExpectedSuccessor,
           InitialWorkerOutstanding, InitialGeneration, NextGeneration
  <1>23. CASE RejectMismatchedPredecessorReceipt
    BY <1>23, SMTT(45)
       DEF TypedRolloverSafetyInvariant,
           TypedRolloverTypeInvariant, ReceiptLifecycleInvariant,
           FinalSealRejectsLateEnqueueInvariant,
           ReceiptExactOwnerAndPredecessorInvariant,
           RetainedWrapperIdentityInvariant,
           TransitionAuthorityLifecycleInvariant,
           OrdinaryChangedRosterTransitionInvariant,
           LiveChangedRosterNeedsTypedReceiptInvariant,
           TypedTransitionAtomicClearInvariant,
           SameRosterTransportPreservationInvariant,
           SameRosterRetryPreservationInvariant,
           RetryChunkHasLiveOwnerInvariant,
           DurableHighWaterFailClosedInvariant,
           HighWaterAheadSnapshotInvariant,
           TornMidpointOpenRejectionInvariant,
           TornHighWaterHistoryOriginInvariant,
           ForeignOwnerCandidateRejectionInvariant,
           PredecessorMismatchCandidateRejectionInvariant,
           WrongSuccessorCandidateRejectionInvariant,
           FailureLatchInvariant, PredecessorMismatchFailure,
           ValidationCandidatePending, validationHistoryVars,
           validationHistoriesExceptTorn,
           validationHistoriesExceptForeignOwner,
           validationHistoriesExceptPredecessorCandidate,
           validationHistoriesExceptWrongSuccessorCandidate,
           LateOldCallbackIsolationInvariant,
           HighWaterAheadOfLifecycleSnapshot,
           PreparedHighWaterSnapshotMidpoint,
           TornHighWaterSnapshotMidpoint,
           RejectMismatchedPredecessorReceipt,
           ExactPredecessorReceipt, ExactServiceTransportOwnerPair,
           ExactSuccessorConstruction, RosterRelations, ReceiptStages,
           TransitionAuthorities, FailureReasons, OwnerNonces,
           PredecessorMismatchKinds, Contexts, Artifacts, Parents,
           Successors, NoIdentity, ExpectedParent, ExpectedSuccessor,
           InitialWorkerOutstanding, InitialGeneration, NextGeneration
  <1>24. CASE PresentWrongImmediateSuccessor
    BY <1>24, SMTT(45)
       DEF TypedRolloverSafetyInvariant,
           TypedRolloverTypeInvariant, ReceiptLifecycleInvariant,
           FinalSealRejectsLateEnqueueInvariant,
           ReceiptExactOwnerAndPredecessorInvariant,
           RetainedWrapperIdentityInvariant,
           TransitionAuthorityLifecycleInvariant,
           OrdinaryChangedRosterTransitionInvariant,
           LiveChangedRosterNeedsTypedReceiptInvariant,
           TypedTransitionAtomicClearInvariant,
           SameRosterTransportPreservationInvariant,
           SameRosterRetryPreservationInvariant,
           RetryChunkHasLiveOwnerInvariant,
           DurableHighWaterFailClosedInvariant,
           HighWaterAheadSnapshotInvariant,
           TornMidpointOpenRejectionInvariant,
           TornHighWaterHistoryOriginInvariant,
           ForeignOwnerCandidateRejectionInvariant,
           PredecessorMismatchCandidateRejectionInvariant,
           WrongSuccessorCandidateRejectionInvariant,
           FailureLatchInvariant, PredecessorMismatchFailure,
           ValidationCandidatePending, validationHistoryVars,
           validationHistoriesExceptTorn,
           validationHistoriesExceptForeignOwner,
           validationHistoriesExceptPredecessorCandidate,
           validationHistoriesExceptWrongSuccessorCandidate,
           LateOldCallbackIsolationInvariant,
           HighWaterAheadOfLifecycleSnapshot,
           PreparedHighWaterSnapshotMidpoint,
           TornHighWaterSnapshotMidpoint,
           PresentWrongImmediateSuccessor,
           ExactPredecessorReceipt, ExactServiceTransportOwnerPair,
           ExactSuccessorConstruction, RosterRelations, ReceiptStages,
           TransitionAuthorities, FailureReasons, OwnerNonces,
           PredecessorMismatchKinds, Contexts, Artifacts, Parents,
           Successors, NoIdentity, ExpectedParent, ExpectedSuccessor,
           InitialWorkerOutstanding, InitialGeneration, NextGeneration
  <1>25. CASE RejectWrongImmediateSuccessor
    BY <1>25, SMTT(45)
       DEF TypedRolloverSafetyInvariant,
           TypedRolloverTypeInvariant, ReceiptLifecycleInvariant,
           FinalSealRejectsLateEnqueueInvariant,
           ReceiptExactOwnerAndPredecessorInvariant,
           RetainedWrapperIdentityInvariant,
           TransitionAuthorityLifecycleInvariant,
           OrdinaryChangedRosterTransitionInvariant,
           LiveChangedRosterNeedsTypedReceiptInvariant,
           TypedTransitionAtomicClearInvariant,
           SameRosterTransportPreservationInvariant,
           SameRosterRetryPreservationInvariant,
           RetryChunkHasLiveOwnerInvariant,
           DurableHighWaterFailClosedInvariant,
           HighWaterAheadSnapshotInvariant,
           TornMidpointOpenRejectionInvariant,
           TornHighWaterHistoryOriginInvariant,
           ForeignOwnerCandidateRejectionInvariant,
           PredecessorMismatchCandidateRejectionInvariant,
           WrongSuccessorCandidateRejectionInvariant,
           FailureLatchInvariant, PredecessorMismatchFailure,
           ValidationCandidatePending, validationHistoryVars,
           validationHistoriesExceptTorn,
           validationHistoriesExceptForeignOwner,
           validationHistoriesExceptPredecessorCandidate,
           validationHistoriesExceptWrongSuccessorCandidate,
           LateOldCallbackIsolationInvariant,
           HighWaterAheadOfLifecycleSnapshot,
           PreparedHighWaterSnapshotMidpoint,
           TornHighWaterSnapshotMidpoint,
           RejectWrongImmediateSuccessor,
           ExactPredecessorReceipt, ExactServiceTransportOwnerPair,
           ExactSuccessorConstruction, RosterRelations, ReceiptStages,
           TransitionAuthorities, FailureReasons, OwnerNonces,
           PredecessorMismatchKinds, Contexts, Artifacts, Parents,
           Successors, NoIdentity, ExpectedParent, ExpectedSuccessor,
           InitialWorkerOutstanding, InitialGeneration, NextGeneration
  <1> QED BY <1>1, <1>2, <1>3, <1>4, <1>5, <1>6, <1>7,
               <1>8, <1>9, <1>10, <1>11, <1>12, <1>13, <1>14,
               <1>15, <1>16, <1>17, <1>18, <1>19, <1>20,
               <1>21, <1>22, <1>23, <1>24, <1>25
       DEF Next

THEOREM TypedRolloverSpecAlwaysSafe ==
  TypedRolloverSpec => []TypedRolloverSafetyInvariant
PROOF
  <1>1. Init => TypedRolloverSafetyInvariant
    BY TypedRolloverInitEstablishesSafety
  <1>2. ASSUME TypedRolloverSafetyInvariant,
                [Next]_typedRolloverVars
         PROVE TypedRolloverSafetyInvariant'
      <2>1. CASE Next
        BY <1>2, <2>1, TypedRolloverNextPreservesSafety
      <2>2. CASE UNCHANGED typedRolloverVars
        <3>1. /\ targetRoster' = targetRoster
               /\ finalityValidated' = finalityValidated
               /\ workerIngressClosed' = workerIngressClosed
               /\ workerOutstanding' = workerOutstanding
               /\ ownerSealed' = ownerSealed
               /\ constructionParent' = constructionParent
               /\ constructionSuccessor' = constructionSuccessor
               /\ receiptStage' = receiptStage
               /\ receiptOwnerNonce' = receiptOwnerNonce
               /\ receiptContext' = receiptContext
               /\ receiptArtifact' = receiptArtifact
               /\ retainedSuccessor' = retainedSuccessor
               /\ receiptConsumeCount' = receiptConsumeCount
               /\ serviceGeneration' = serviceGeneration
               /\ durableHighWater' = durableHighWater
               /\ responderActive' = responderActive
               /\ responderWritable' = responderWritable
               /\ responderAuthorized' = responderAuthorized
               /\ retryableChunk' = retryableChunk
               /\ successorActive' = successorActive
               /\ transitionAuthority' = transitionAuthority
               /\ forcedTransitionUsed' = forcedTransitionUsed
               /\ transitionedFromLiveResponder' =
                    transitionedFromLiveResponder
               /\ ordinaryLiveTransitionRejected' =
                    ordinaryLiveTransitionRejected
               /\ foreignReceiptRejected' = foreignReceiptRejected
               /\ lateEnqueueRejected' = lateEnqueueRejected
               /\ lateOldCallbackObserved' = lateOldCallbackObserved
               /\ successorCursor' = successorCursor
               /\ restartRequired' = restartRequired
               /\ failureReason' = failureReason
               /\ tornMidpointOpenRejected' = tornMidpointOpenRejected
               /\ foreignReceiptCandidatePresent' =
                    foreignReceiptCandidatePresent
               /\ tornHighWaterHistory' = tornHighWaterHistory
               /\ foreignReceiptCandidateObserved' =
                    foreignReceiptCandidateObserved
               /\ predecessorMismatchCandidateKind' =
                    predecessorMismatchCandidateKind
               /\ predecessorMismatchObservedKind' =
                    predecessorMismatchObservedKind
               /\ predecessorMismatchRejected' =
                    predecessorMismatchRejected
               /\ wrongSuccessorCandidatePresent' =
                    wrongSuccessorCandidatePresent
               /\ wrongSuccessorCandidateObserved' =
                    wrongSuccessorCandidateObserved
               /\ wrongSuccessorRejected' = wrongSuccessorRejected
          BY <2>2, Isa DEF typedRolloverVars
        <3> QED BY <1>2, <3>1, SMTT(30)
             DEF TypedRolloverSafetyInvariant,
                 TypedRolloverTypeInvariant, ReceiptLifecycleInvariant,
                 FinalSealRejectsLateEnqueueInvariant,
                 ReceiptExactOwnerAndPredecessorInvariant,
                 RetainedWrapperIdentityInvariant,
                 TransitionAuthorityLifecycleInvariant,
                 OrdinaryChangedRosterTransitionInvariant,
                 LiveChangedRosterNeedsTypedReceiptInvariant,
                 TypedTransitionAtomicClearInvariant,
                 SameRosterTransportPreservationInvariant,
                 SameRosterRetryPreservationInvariant,
                 RetryChunkHasLiveOwnerInvariant,
                 DurableHighWaterFailClosedInvariant,
                 HighWaterAheadSnapshotInvariant,
                 TornMidpointOpenRejectionInvariant,
                 TornHighWaterHistoryOriginInvariant,
                 ForeignOwnerCandidateRejectionInvariant,
                 PredecessorMismatchCandidateRejectionInvariant,
                 WrongSuccessorCandidateRejectionInvariant,
                 FailureLatchInvariant,
                 PredecessorMismatchFailure, ValidationCandidatePending,
                 validationHistoryVars, validationHistoriesExceptTorn,
                 validationHistoriesExceptForeignOwner,
                 validationHistoriesExceptPredecessorCandidate,
                 validationHistoriesExceptWrongSuccessorCandidate,
                 LateOldCallbackIsolationInvariant,
                 HighWaterAheadOfLifecycleSnapshot,
                 PreparedHighWaterSnapshotMidpoint,
                 TornHighWaterSnapshotMidpoint,
                 ExactPredecessorReceipt, ExactServiceTransportOwnerPair,
                 ExactSuccessorConstruction
      <2> QED BY <1>2, <2>1, <2>2
           DEF typedRolloverVars
  <1> QED BY <1>1, <1>2, PTL DEF TypedRolloverSpec

(***************************************************************************
The local liveness argument below is deliberately conditional and
failure-free.  In particular, fair fail-stop validation rejection plus
`NoRolloverFailure` excludes every execution in which an invalid candidate is
presented.  The argument therefore proves finite local handoff progress; it
does not prove recovery from validation failure, network delivery, writer
flush, or eventual finality validation.

The control partition is an auxiliary inductive invariant.  Keeping it
separate from `TypedRolloverSafetyInvariant` avoids treating unreachable
states admitted by the state invariant as reachable handoff stages.
***************************************************************************)

RolloverHealthy ==
  /\ failureReason = "None"
  /\ ~restartRequired

ChangedRosterValidated ==
  /\ targetRoster = "ChangedRoster"
  /\ finalityValidated

PreValidationControl ==
  /\ ~finalityValidated
  /\ ~workerIngressClosed
  /\ workerOutstanding = InitialWorkerOutstanding
  /\ ~ownerSealed
  /\ constructionParent = NoIdentity
  /\ constructionSuccessor = NoIdentity
  /\ receiptStage = "Absent"
  /\ receiptOwnerNonce = NoIdentity
  /\ receiptContext = NoIdentity
  /\ receiptArtifact = NoIdentity
  /\ retainedSuccessor = NoIdentity
  /\ receiptConsumeCount = 0
  /\ serviceGeneration = InitialGeneration
  /\ durableHighWater = InitialGeneration
  /\ ~successorActive
  /\ transitionAuthority = "None"
  /\ ~forcedTransitionUsed

ChangedRolloverPreSealShape ==
  /\ RolloverHealthy
  /\ ChangedRosterValidated
  /\ ~ownerSealed
  /\ constructionParent = NoIdentity
  /\ constructionSuccessor = NoIdentity
  /\ receiptStage = "Absent"
  /\ receiptOwnerNonce = NoIdentity
  /\ receiptContext = NoIdentity
  /\ receiptArtifact = NoIdentity
  /\ retainedSuccessor = NoIdentity
  /\ receiptConsumeCount = 0
  /\ serviceGeneration = InitialGeneration
  /\ durableHighWater = InitialGeneration
  /\ ~successorActive
  /\ transitionAuthority = "None"
  /\ ~forcedTransitionUsed

ChangedRolloverRank8 ==
  /\ ChangedRolloverPreSealShape
  /\ ~workerIngressClosed
  /\ workerOutstanding = InitialWorkerOutstanding

ChangedRolloverRank7 ==
  /\ ChangedRolloverPreSealShape
  /\ workerIngressClosed
  /\ workerOutstanding = InitialWorkerOutstanding

ChangedRolloverRank6 ==
  /\ ChangedRolloverPreSealShape
  /\ workerIngressClosed
  /\ workerOutstanding = 1

ChangedRolloverRank5 ==
  /\ ChangedRolloverPreSealShape
  /\ workerIngressClosed
  /\ workerOutstanding = 0

ChangedRolloverRank4 ==
  /\ RolloverHealthy
  /\ ChangedRosterValidated
  /\ FinalExactOutputSeal
  /\ receiptStage = "Minted"
  /\ ExactPredecessorReceipt
  /\ constructionParent = NoIdentity
  /\ constructionSuccessor = NoIdentity
  /\ retainedSuccessor = NoIdentity
  /\ receiptConsumeCount = 0
  /\ serviceGeneration = InitialGeneration
  /\ durableHighWater = InitialGeneration
  /\ ~successorActive
  /\ transitionAuthority = "None"
  /\ ~forcedTransitionUsed
  /\ ~ValidationCandidatePending

ChangedRolloverRank3 ==
  /\ RolloverHealthy
  /\ ChangedRosterValidated
  /\ FinalExactOutputSeal
  /\ receiptStage = "Minted"
  /\ ExactPredecessorReceipt
  /\ ExactSuccessorConstruction
  /\ retainedSuccessor = NoIdentity
  /\ receiptConsumeCount = 0
  /\ serviceGeneration = InitialGeneration
  /\ durableHighWater = InitialGeneration
  /\ ~successorActive
  /\ transitionAuthority = "None"
  /\ ~forcedTransitionUsed
  /\ ~ValidationCandidatePending

ChangedRolloverRank2 ==
  /\ RolloverHealthy
  /\ ChangedRosterValidated
  /\ FinalExactOutputSeal
  /\ ExactRetainedMergeSidecars
  /\ receiptConsumeCount = 0
  /\ serviceGeneration = InitialGeneration
  /\ durableHighWater = InitialGeneration
  /\ ~successorActive
  /\ transitionAuthority = "None"
  /\ ~forcedTransitionUsed
  /\ ~ValidationCandidatePending

ChangedRolloverRank1 ==
  /\ ChangedRosterValidated
  /\ FinalExactOutputSeal
  /\ ExactRetainedMergeSidecars
  /\ PreparedHighWaterSnapshotMidpoint
  /\ receiptConsumeCount = 0
  /\ ~successorActive
  /\ transitionAuthority = "None"
  /\ ~forcedTransitionUsed
  /\ ~ValidationCandidatePending

ChangedRolloverRank1Exit ==
  \/ ChangedRosterSuccessorActiveWithoutRestart
  \/ ~RolloverHealthy
  \/ ValidationCandidatePending

ChangedRolloverRank2Exit ==
  \/ ChangedRolloverRank1
  \/ ChangedRolloverRank1Exit

ChangedRolloverRank3Exit ==
  \/ ChangedRolloverRank2
  \/ ChangedRolloverRank2Exit

ChangedRolloverRank4Exit ==
  \/ ChangedRolloverRank3
  \/ ChangedRolloverRank3Exit

ChangedRolloverRank5Exit ==
  \/ ChangedRolloverRank4
  \/ ChangedRolloverRank4Exit

ChangedRolloverRank6Exit ==
  \/ ChangedRolloverRank5
  \/ ChangedRolloverRank5Exit

ChangedRolloverRank7Exit ==
  \/ ChangedRolloverRank6
  \/ ChangedRolloverRank6Exit

ChangedRolloverRank8Exit ==
  \/ ChangedRolloverRank7
  \/ ChangedRolloverRank7Exit

ChangedRolloverControlInvariant ==
  /\ (~finalityValidated => PreValidationControl)
  /\ (ChangedRosterValidated =>
        \/ restartRequired
        \/ ValidationCandidatePending
        \/ ChangedRosterSuccessorActiveWithoutRestart
        \/ ChangedRolloverRank1
        \/ ChangedRolloverRank2
        \/ ChangedRolloverRank3
        \/ ChangedRolloverRank4
        \/ ChangedRolloverRank5
        \/ ChangedRolloverRank6
        \/ ChangedRolloverRank7
        \/ ChangedRolloverRank8)

THEOREM TypedRolloverInitEstablishesControl ==
  Init => ChangedRolloverControlInvariant
BY SMTT(30)
   DEF Init, ChangedRolloverControlInvariant, PreValidationControl,
       ChangedRosterValidated, ChangedRolloverRank1,
       ChangedRolloverRank2, ChangedRolloverRank3,
       ChangedRolloverRank4, ChangedRolloverRank5,
       ChangedRolloverRank6, ChangedRolloverRank7,
       ChangedRolloverRank8, ChangedRolloverPreSealShape,
       ChangedRosterSuccessorActiveWithoutRestart,
       RolloverHealthy, ValidationCandidatePending,
       ExactRetainedMergeSidecars, ExactPredecessorReceipt,
       ExactServiceTransportOwnerPair, ExactSuccessorConstruction,
       FinalExactOutputSeal, PreparedHighWaterSnapshotMidpoint,
       HighWaterAheadOfLifecycleSnapshot, NoIdentity,
       InitialWorkerOutstanding, InitialGeneration, NextGeneration

(***************************************************************************
This action kernel unfolds every one of the fixed model's 25 `Next` branches.
Failures enter the explicit restart arm, candidate presentations enter the
candidate arm, and the remaining branches preserve or descend the rank.
***************************************************************************)
THEOREM TypedRolloverNextPreservesControl ==
  /\ TypedRolloverSafetyInvariant
  /\ ChangedRolloverControlInvariant
  /\ Next
  => ChangedRolloverControlInvariant'
BY SMTT(180)
   DEF ChangedRolloverControlInvariant, PreValidationControl,
       ChangedRosterValidated, ChangedRolloverRank1,
       ChangedRolloverRank2, ChangedRolloverRank3,
       ChangedRolloverRank4, ChangedRolloverRank5,
       ChangedRolloverRank6, ChangedRolloverRank7,
       ChangedRolloverRank8, ChangedRolloverPreSealShape,
       ChangedRosterSuccessorActiveWithoutRestart,
       RolloverHealthy, TypedRolloverSafetyInvariant,
       TypedRolloverTypeInvariant, ReceiptLifecycleInvariant,
       FinalSealRejectsLateEnqueueInvariant,
       ReceiptExactOwnerAndPredecessorInvariant,
       RetainedWrapperIdentityInvariant,
       TransitionAuthorityLifecycleInvariant,
       TypedTransitionAtomicClearInvariant,
       DurableHighWaterFailClosedInvariant,
       HighWaterAheadSnapshotInvariant,
       ForeignOwnerCandidateRejectionInvariant,
       PredecessorMismatchCandidateRejectionInvariant,
       WrongSuccessorCandidateRejectionInvariant,
       FailureLatchInvariant, ValidationCandidatePending,
       PredecessorMismatchFailure, Next, ValidateFinality,
       CloseWorkerIngress, ClearOneWorkerExactOutput,
       SealAppliedHeightOutputHandoff, RejectLateExactOutputEnqueue,
       BeginExactSuccessorConstruction,
       PresentSameContextForeignOwnerReceipt,
       RejectSameContextForeignOwnerReceipt,
       PresentMismatchedPredecessorContextReceipt,
       PresentMismatchedPredecessorArtifactReceipt,
       RejectMismatchedPredecessorReceipt,
       PresentWrongImmediateSuccessor, RejectWrongImmediateSuccessor,
       ConsumeReceiptIntoRetainedMergeSidecars,
       PersistNextServiceHighWater, FailNextServiceHighWaterPersistence,
       FailLifecycleSnapshotAfterHighWaterPersistence,
       CrashAtHighWaterAheadSnapshot, RejectTornHighWaterSnapshotOpen,
       RejectOrdinaryLiveChangedRosterTransition,
       QuiesceChangedRosterResponder, TypedChangedRosterTransition,
       SameRosterRetainedTransportRollover,
       ObserveLateOldWriterCallback, CrashWithRolledBackHighWater,
       validationHistoryVars, validationHistoriesExceptTorn,
       validationHistoriesExceptForeignOwner,
       validationHistoriesExceptPredecessorCandidate,
       validationHistoriesExceptWrongSuccessorCandidate,
       ExactRetainedMergeSidecars, ExactPredecessorReceipt,
       ExactServiceTransportOwnerPair, ExactSuccessorConstruction,
       FinalExactOutputSeal, PreparedHighWaterSnapshotMidpoint,
       TornHighWaterSnapshotMidpoint,
       HighWaterAheadOfLifecycleSnapshot,
       ResponderBlocksOrdinaryTransition, RosterRelations, ReceiptStages,
       TransitionAuthorities, FailureReasons, OwnerNonces,
       PredecessorMismatchKinds, Contexts, Artifacts, Parents, Successors,
       NoIdentity, ServiceOwnerNonce, TransportOwnerNonce,
       ForeignOwnerNonce, ExpectedContext, ExpectedArtifact,
       ExpectedParent, ExpectedSuccessor, InitialWorkerOutstanding,
       InitialGeneration, NextGeneration

THEOREM TypedRolloverStutterPreservesControl ==
  /\ ChangedRolloverControlInvariant
  /\ UNCHANGED typedRolloverVars
  => ChangedRolloverControlInvariant'
BY SMTT(30)
   DEF ChangedRolloverControlInvariant, PreValidationControl,
       ChangedRosterValidated, ChangedRolloverRank1,
       ChangedRolloverRank2, ChangedRolloverRank3,
       ChangedRolloverRank4, ChangedRolloverRank5,
       ChangedRolloverRank6, ChangedRolloverRank7,
       ChangedRolloverRank8, ChangedRolloverPreSealShape,
       ChangedRosterSuccessorActiveWithoutRestart,
       RolloverHealthy, ValidationCandidatePending,
       ExactRetainedMergeSidecars, ExactPredecessorReceipt,
       ExactServiceTransportOwnerPair, ExactSuccessorConstruction,
       FinalExactOutputSeal, PreparedHighWaterSnapshotMidpoint,
       HighWaterAheadOfLifecycleSnapshot, typedRolloverVars

THEOREM TypedRolloverBracketPreservesControl ==
  /\ TypedRolloverSafetyInvariant
  /\ ChangedRolloverControlInvariant
  /\ [Next]_typedRolloverVars
  => ChangedRolloverControlInvariant'
BY TypedRolloverNextPreservesControl,
   TypedRolloverStutterPreservesControl, SMTT(10)
   DEF typedRolloverVars

THEOREM TypedRolloverSpecAlwaysControl ==
  TypedRolloverSpec => []ChangedRolloverControlInvariant
BY TypedRolloverInitEstablishesControl,
   TypedRolloverSpecAlwaysSafe,
   TypedRolloverBracketPreservesControl, PTL
   DEF TypedRolloverSpec

(***************************************************************************
A pending invalid candidate is persistent until its matching fail-stop
rejection.  The rejection action is continuously enabled and weakly fair.
Consequently a pending candidate leads to restart; `NoRolloverFailure` then
excludes candidate presentation from every behavior admitted by the
conditional liveness theorem.
***************************************************************************)

THEOREM ValidationCandidatesPersistOrFail ==
  /\ TypedRolloverSafetyInvariant
  /\ [Next]_typedRolloverVars
  =>
    /\ (foreignReceiptCandidatePresent =>
          foreignReceiptCandidatePresent' \/ restartRequired')
    /\ ((predecessorMismatchCandidateKind # "NoMismatch") =>
          \/ predecessorMismatchCandidateKind' # "NoMismatch"
          \/ restartRequired')
    /\ (wrongSuccessorCandidatePresent =>
          wrongSuccessorCandidatePresent' \/ restartRequired')
BY TypedRolloverNextSatisfiesActionSafety, SMTT(30)
   DEF ForeignOwnerCandidateStepSafety,
       PredecessorMismatchCandidateStepSafety,
       WrongSuccessorCandidateStepSafety, typedRolloverVars

THEOREM ValidationCandidateRejectActionsEnabled ==
  TypedRolloverSafetyInvariant =>
    /\ (foreignReceiptCandidatePresent =>
          ENABLED
            <<RejectSameContextForeignOwnerReceipt>>_typedRolloverVars)
    /\ ((predecessorMismatchCandidateKind # "NoMismatch") =>
          ENABLED
            <<RejectMismatchedPredecessorReceipt>>_typedRolloverVars)
    /\ (wrongSuccessorCandidatePresent =>
          ENABLED
            <<RejectWrongImmediateSuccessor>>_typedRolloverVars)
BY ExpandENABLED, SharedOwnerNonceDistinguishesForeignService, SMTT(30)
   DEF TypedRolloverSafetyInvariant, TypedRolloverTypeInvariant,
       ForeignOwnerCandidateRejectionInvariant,
       PredecessorMismatchCandidateRejectionInvariant,
       WrongSuccessorCandidateRejectionInvariant,
       RejectSameContextForeignOwnerReceipt,
       RejectMismatchedPredecessorReceipt,
       RejectWrongImmediateSuccessor, PredecessorMismatchFailure,
       PredecessorMismatchKinds, typedRolloverVars

THEOREM ValidationCandidateRejectActionsExit ==
  /\ (<<RejectSameContextForeignOwnerReceipt>>_typedRolloverVars =>
        restartRequired')
  /\ (<<RejectMismatchedPredecessorReceipt>>_typedRolloverVars =>
        restartRequired')
  /\ (<<RejectWrongImmediateSuccessor>>_typedRolloverVars =>
        restartRequired')
BY DEF RejectSameContextForeignOwnerReceipt,
       RejectMismatchedPredecessorReceipt,
       RejectWrongImmediateSuccessor

THEOREM ValidationCandidatesLeadToRestart ==
  ResponsiveTypedRolloverSpec =>
    /\ (foreignReceiptCandidatePresent ~> restartRequired)
    /\ ((predecessorMismatchCandidateKind # "NoMismatch")
          ~> restartRequired)
    /\ (wrongSuccessorCandidatePresent ~> restartRequired)
BY TypedRolloverSpecAlwaysSafe, ValidationCandidatesPersistOrFail,
   ValidationCandidateRejectActionsEnabled,
   ValidationCandidateRejectActionsExit, PTL
   DEF ResponsiveTypedRolloverSpec

THEOREM ValidationCandidateLeadsToRestart ==
  ResponsiveTypedRolloverSpec =>
    (ValidationCandidatePending ~> restartRequired)
BY ValidationCandidatesLeadToRestart, PTL
   DEF ValidationCandidatePending

THEOREM NoFailureExcludesValidationCandidates ==
  /\ ResponsiveTypedRolloverSpec
  /\ NoRolloverFailure
  => []~ValidationCandidatePending
BY ValidationCandidateLeadsToRestart, PTL
   DEF NoRolloverFailure

(***************************************************************************
One reusable bracket kernel covers all eight healthy ranks.  Every step either
preserves the current rank, descends, presents a validation candidate, or
enters the failure arm.  The latter two exits are removed only later, by the
deduced candidate-exclusion theorem and the explicit no-failure hypothesis.
***************************************************************************)
THEOREM ChangedRolloverRankBracketClosure ==
  /\ TypedRolloverSafetyInvariant
  /\ ChangedRolloverControlInvariant
  /\ [Next]_typedRolloverVars
  =>
    /\ (ChangedRolloverRank1 =>
          ChangedRolloverRank1' \/ ChangedRolloverRank1Exit')
    /\ (ChangedRolloverRank2 =>
          ChangedRolloverRank2' \/ ChangedRolloverRank2Exit')
    /\ (ChangedRolloverRank3 =>
          ChangedRolloverRank3' \/ ChangedRolloverRank3Exit')
    /\ (ChangedRolloverRank4 =>
          ChangedRolloverRank4' \/ ChangedRolloverRank4Exit')
    /\ (ChangedRolloverRank5 =>
          ChangedRolloverRank5' \/ ChangedRolloverRank5Exit')
    /\ (ChangedRolloverRank6 =>
          ChangedRolloverRank6' \/ ChangedRolloverRank6Exit')
    /\ (ChangedRolloverRank7 =>
          ChangedRolloverRank7' \/ ChangedRolloverRank7Exit')
    /\ (ChangedRolloverRank8 =>
          ChangedRolloverRank8' \/ ChangedRolloverRank8Exit')
BY TypedRolloverBracketPreservesControl, SMTT(180)
   DEF ChangedRolloverControlInvariant, ChangedRolloverRank1,
       ChangedRolloverRank2, ChangedRolloverRank3,
       ChangedRolloverRank4, ChangedRolloverRank5,
       ChangedRolloverRank6, ChangedRolloverRank7,
       ChangedRolloverRank8, ChangedRolloverRank1Exit,
       ChangedRolloverRank2Exit, ChangedRolloverRank3Exit,
       ChangedRolloverRank4Exit, ChangedRolloverRank5Exit,
       ChangedRolloverRank6Exit, ChangedRolloverRank7Exit,
       ChangedRolloverRank8Exit, ChangedRolloverPreSealShape,
       ChangedRosterValidated, ChangedRosterSuccessorActiveWithoutRestart,
       RolloverHealthy, TypedRolloverSafetyInvariant,
       FailureLatchInvariant, ValidationCandidatePending,
       Next, ValidateFinality, CloseWorkerIngress,
       ClearOneWorkerExactOutput, SealAppliedHeightOutputHandoff,
       RejectLateExactOutputEnqueue, BeginExactSuccessorConstruction,
       PresentSameContextForeignOwnerReceipt,
       RejectSameContextForeignOwnerReceipt,
       PresentMismatchedPredecessorContextReceipt,
       PresentMismatchedPredecessorArtifactReceipt,
       RejectMismatchedPredecessorReceipt,
       PresentWrongImmediateSuccessor, RejectWrongImmediateSuccessor,
       ConsumeReceiptIntoRetainedMergeSidecars,
       PersistNextServiceHighWater, FailNextServiceHighWaterPersistence,
       FailLifecycleSnapshotAfterHighWaterPersistence,
       CrashAtHighWaterAheadSnapshot, RejectTornHighWaterSnapshotOpen,
       RejectOrdinaryLiveChangedRosterTransition,
       QuiesceChangedRosterResponder, TypedChangedRosterTransition,
       SameRosterRetainedTransportRollover,
       ObserveLateOldWriterCallback, CrashWithRolledBackHighWater,
       ExactRetainedMergeSidecars, ExactPredecessorReceipt,
       ExactServiceTransportOwnerPair, ExactSuccessorConstruction,
       FinalExactOutputSeal, PreparedHighWaterSnapshotMidpoint,
       HighWaterAheadOfLifecycleSnapshot,
       ResponderBlocksOrdinaryTransition, validationHistoryVars,
       validationHistoriesExceptTorn,
       validationHistoriesExceptForeignOwner,
       validationHistoriesExceptPredecessorCandidate,
       validationHistoriesExceptWrongSuccessorCandidate,
       typedRolloverVars, InitialWorkerOutstanding, InitialGeneration,
       NextGeneration, NoIdentity, ExpectedContext, ExpectedArtifact,
       ExpectedParent, ExpectedSuccessor, ServiceOwnerNonce,
       TransportOwnerNonce, ForeignOwnerNonce

THEOREM ChangedRolloverRankFairActionsEnabled ==
  /\ (ChangedRolloverRank1 =>
        ENABLED <<TypedChangedRosterTransition>>_typedRolloverVars)
  /\ (ChangedRolloverRank2 =>
        ENABLED <<PersistNextServiceHighWater>>_typedRolloverVars)
  /\ (ChangedRolloverRank3 =>
        ENABLED
          <<ConsumeReceiptIntoRetainedMergeSidecars>>_typedRolloverVars)
  /\ (ChangedRolloverRank4 =>
        ENABLED <<BeginExactSuccessorConstruction>>_typedRolloverVars)
  /\ (ChangedRolloverRank5 =>
        ENABLED <<SealAppliedHeightOutputHandoff>>_typedRolloverVars)
  /\ (ChangedRolloverRank6 =>
        ENABLED <<ClearOneWorkerExactOutput>>_typedRolloverVars)
  /\ (ChangedRolloverRank7 =>
        ENABLED <<ClearOneWorkerExactOutput>>_typedRolloverVars)
  /\ (ChangedRolloverRank8 =>
        ENABLED <<CloseWorkerIngress>>_typedRolloverVars)
BY ExpandENABLED, SMTT(60)
   DEF ChangedRolloverRank1, ChangedRolloverRank2,
       ChangedRolloverRank3, ChangedRolloverRank4,
       ChangedRolloverRank5, ChangedRolloverRank6,
       ChangedRolloverRank7, ChangedRolloverRank8,
       ChangedRolloverPreSealShape, ChangedRosterValidated,
       TypedChangedRosterTransition, PersistNextServiceHighWater,
       ConsumeReceiptIntoRetainedMergeSidecars,
       BeginExactSuccessorConstruction,
       SealAppliedHeightOutputHandoff, ClearOneWorkerExactOutput,
       CloseWorkerIngress, PreparedHighWaterSnapshotMidpoint,
       ExactRetainedMergeSidecars, ExactPredecessorReceipt,
       ExactSuccessorConstruction, RolloverHealthy,
       ValidationCandidatePending, typedRolloverVars

THEOREM ChangedRolloverRankFairActionsExit ==
  /\ (/\ ChangedRolloverRank1
      /\ <<TypedChangedRosterTransition>>_typedRolloverVars
      => ChangedRolloverRank1Exit')
  /\ (/\ ChangedRolloverRank2
      /\ <<PersistNextServiceHighWater>>_typedRolloverVars
      => ChangedRolloverRank2Exit')
  /\ (/\ ChangedRolloverRank3
      /\ <<ConsumeReceiptIntoRetainedMergeSidecars>>_typedRolloverVars
      => ChangedRolloverRank3Exit')
  /\ (/\ ChangedRolloverRank4
      /\ <<BeginExactSuccessorConstruction>>_typedRolloverVars
      => ChangedRolloverRank4Exit')
  /\ (/\ ChangedRolloverRank5
      /\ <<SealAppliedHeightOutputHandoff>>_typedRolloverVars
      => ChangedRolloverRank5Exit')
  /\ (/\ ChangedRolloverRank6
      /\ <<ClearOneWorkerExactOutput>>_typedRolloverVars
      => ChangedRolloverRank6Exit')
  /\ (/\ ChangedRolloverRank7
      /\ <<ClearOneWorkerExactOutput>>_typedRolloverVars
      => ChangedRolloverRank7Exit')
  /\ (/\ ChangedRolloverRank8
      /\ <<CloseWorkerIngress>>_typedRolloverVars
      => ChangedRolloverRank8Exit')
BY SMTT(60)
   DEF ChangedRolloverRank1, ChangedRolloverRank2,
       ChangedRolloverRank3, ChangedRolloverRank4,
       ChangedRolloverRank5, ChangedRolloverRank6,
       ChangedRolloverRank7, ChangedRolloverRank8,
       ChangedRolloverRank1Exit, ChangedRolloverRank2Exit,
       ChangedRolloverRank3Exit, ChangedRolloverRank4Exit,
       ChangedRolloverRank5Exit, ChangedRolloverRank6Exit,
       ChangedRolloverRank7Exit, ChangedRolloverRank8Exit,
       ChangedRolloverPreSealShape, ChangedRosterValidated,
       ChangedRosterSuccessorActiveWithoutRestart,
       TypedChangedRosterTransition, PersistNextServiceHighWater,
       ConsumeReceiptIntoRetainedMergeSidecars,
       BeginExactSuccessorConstruction,
       SealAppliedHeightOutputHandoff, ClearOneWorkerExactOutput,
       CloseWorkerIngress, FinalExactOutputSeal,
       ExactRetainedMergeSidecars, ExactPredecessorReceipt,
       ExactServiceTransportOwnerPair, ExactSuccessorConstruction,
       PreparedHighWaterSnapshotMidpoint,
       HighWaterAheadOfLifecycleSnapshot, RolloverHealthy,
       ValidationCandidatePending, InitialWorkerOutstanding,
       ServiceOwnerNonce, TransportOwnerNonce,
       ExpectedContext, ExpectedArtifact, ExpectedParent,
       ExpectedSuccessor

THEOREM ChangedRolloverRanksLeadToExit ==
  ResponsiveTypedRolloverSpec =>
    /\ (ChangedRolloverRank1 ~> ChangedRolloverRank1Exit)
    /\ (ChangedRolloverRank2 ~> ChangedRolloverRank2Exit)
    /\ (ChangedRolloverRank3 ~> ChangedRolloverRank3Exit)
    /\ (ChangedRolloverRank4 ~> ChangedRolloverRank4Exit)
    /\ (ChangedRolloverRank5 ~> ChangedRolloverRank5Exit)
    /\ (ChangedRolloverRank6 ~> ChangedRolloverRank6Exit)
    /\ (ChangedRolloverRank7 ~> ChangedRolloverRank7Exit)
    /\ (ChangedRolloverRank8 ~> ChangedRolloverRank8Exit)
BY TypedRolloverSpecAlwaysSafe, TypedRolloverSpecAlwaysControl,
   ChangedRolloverRankBracketClosure,
   ChangedRolloverRankFairActionsEnabled,
   ChangedRolloverRankFairActionsExit, PTL
   DEF ResponsiveTypedRolloverSpec

THEOREM FailureFreeChangedRolloverRanksLeadToGoal ==
  /\ ResponsiveTypedRolloverSpec
  /\ NoRolloverFailure
  =>
    /\ (ChangedRolloverRank1
          ~> ChangedRosterSuccessorActiveWithoutRestart)
    /\ (ChangedRolloverRank2
          ~> ChangedRosterSuccessorActiveWithoutRestart)
    /\ (ChangedRolloverRank3
          ~> ChangedRosterSuccessorActiveWithoutRestart)
    /\ (ChangedRolloverRank4
          ~> ChangedRosterSuccessorActiveWithoutRestart)
    /\ (ChangedRolloverRank5
          ~> ChangedRosterSuccessorActiveWithoutRestart)
    /\ (ChangedRolloverRank6
          ~> ChangedRosterSuccessorActiveWithoutRestart)
    /\ (ChangedRolloverRank7
          ~> ChangedRosterSuccessorActiveWithoutRestart)
    /\ (ChangedRolloverRank8
          ~> ChangedRosterSuccessorActiveWithoutRestart)
BY ChangedRolloverRanksLeadToExit,
   NoFailureExcludesValidationCandidates, PTL
   DEF ChangedRolloverRank8Exit, ChangedRolloverRank7Exit,
       ChangedRolloverRank6Exit, ChangedRolloverRank5Exit,
       ChangedRolloverRank4Exit, ChangedRolloverRank3Exit,
       ChangedRolloverRank2Exit, ChangedRolloverRank1Exit,
       NoRolloverFailure, RolloverHealthy

THEOREM HealthyCandidateFreeChangedRosterHasControlRank ==
  /\ ChangedRolloverControlInvariant
  /\ ChangedRosterValidated
  /\ RolloverHealthy
  /\ ~ValidationCandidatePending
  =>
    \/ ChangedRosterSuccessorActiveWithoutRestart
    \/ ChangedRolloverRank1
    \/ ChangedRolloverRank2
    \/ ChangedRolloverRank3
    \/ ChangedRolloverRank4
    \/ ChangedRolloverRank5
    \/ ChangedRolloverRank6
    \/ ChangedRolloverRank7
    \/ ChangedRolloverRank8
BY DEF ChangedRolloverControlInvariant

THEOREM ConditionalResponsiveChangedRosterRolloverLiveness ==
  /\ ResponsiveTypedRolloverSpec
  /\ NoRolloverFailure
  =>
    (ChangedRosterValidated
       ~> ChangedRosterSuccessorActiveWithoutRestart)
BY TypedRolloverSpecAlwaysControl,
   NoFailureExcludesValidationCandidates,
   HealthyCandidateFreeChangedRosterHasControlRank,
   FailureFreeChangedRolloverRanksLeadToGoal, PTL
   DEF NoRolloverFailure, RolloverHealthy

THEOREM ResponsiveChangedRosterRolloverLivenessFromWeakFairness ==
  ResponsiveTypedRolloverSpec =>
    ResponsiveChangedRosterRolloverLiveness
BY ConditionalResponsiveChangedRosterRolloverLiveness, PTL
   DEF ResponsiveChangedRosterRolloverLiveness,
       ChangedRosterValidated

=============================================================================
