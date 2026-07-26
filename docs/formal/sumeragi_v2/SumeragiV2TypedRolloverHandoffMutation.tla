---- MODULE SumeragiV2TypedRolloverHandoffMutation ----
EXTENDS SumeragiV2TypedRolloverHandoff

(***************************************************************************
Independent adversarial substitutions for the move-only rollover boundary.

Each configuration enables exactly one defect and checks the fixed safety
invariant: untyped live force, same-context foreign owner nonce, premature
seal/mint, ignored or clean-rejected validation candidates, accepted
predecessor/successor mismatches, late enqueue after sealing, clean persistence
failures, old callback mutation, missing high-water persistence, unsafe
torn-midpoint reopen, or same-roster retry loss.
***************************************************************************)

CONSTANT MutationMode

MutationModes ==
  {"UntypedForce",
   "ForeignOwnerNonce",
   "IgnoreForeignCandidate",
   "CleanForeignOwnerReject",
   "AcceptPredecessorContextMismatch",
   "AcceptPredecessorArtifactMismatch",
   "CleanPredecessorContextReject",
   "CleanPredecessorArtifactReject",
   "PrematureSeal",
   "ForeignSuccessor",
   "CleanWrongSuccessorReject",
   "LateEnqueue",
   "CleanLateEnqueueReject",
   "LateOldCallback",
   "SkipHighWater",
   "CleanHighWaterPersistenceFailure",
   "CleanLifecycleSnapshotPersistenceFailure",
   "OmitLifecycleSnapshotTornHistory",
   "OpenHighWaterAheadSnapshot",
   "LoseSameRosterRetry"}

MutationInit ==
  /\ Init
  /\ MutationMode \in MutationModes

PrematureSealAndMint ==
  /\ receiptStage = "Absent"
  /\ ~ownerSealed
  /\ ~successorActive
  /\ ~restartRequired
  /\ ownerSealed' = TRUE
  /\ receiptStage' = "Minted"
  /\ receiptOwnerNonce' = ServiceOwnerNonce
  /\ receiptContext' = ExpectedContext
  /\ receiptArtifact' = ExpectedArtifact
  /\ UNCHANGED <<targetRoster, finalityValidated, workerIngressClosed,
                 workerOutstanding, constructionParent,
                 constructionSuccessor, retainedSuccessor,
                 receiptConsumeCount, serviceGeneration, durableHighWater,
                 responderActive, responderWritable, responderAuthorized,
                 retryableChunk, successorActive, transitionAuthority,
                 forcedTransitionUsed, transitionedFromLiveResponder,
                 ordinaryLiveTransitionRejected, foreignReceiptRejected,
                 lateEnqueueRejected, lateOldCallbackObserved,
                 successorCursor, restartRequired, failureReason,
                 tornMidpointOpenRejected,
                 foreignReceiptCandidatePresent,
                 validationHistoryVars>>

RetainSameContextForeignOwnerReceipt ==
  /\ receiptStage = "Minted"
  /\ ExactSuccessorConstruction
  /\ foreignReceiptCandidatePresent
  /\ retainedSuccessor = NoIdentity
  /\ ~successorActive
  /\ ~restartRequired
  /\ receiptStage' = "Retained"
  /\ receiptOwnerNonce' = ForeignOwnerNonce
  /\ retainedSuccessor' = ExpectedSuccessor
  /\ foreignReceiptCandidatePresent' = FALSE
  /\ UNCHANGED <<targetRoster, finalityValidated, workerIngressClosed,
                 workerOutstanding, ownerSealed, constructionParent,
                 constructionSuccessor, receiptContext, receiptArtifact,
                 receiptConsumeCount, serviceGeneration, durableHighWater,
                 responderActive, responderWritable, responderAuthorized,
                 retryableChunk, successorActive, transitionAuthority,
                 forcedTransitionUsed, transitionedFromLiveResponder,
                 ordinaryLiveTransitionRejected, foreignReceiptRejected,
                 lateEnqueueRejected, lateOldCallbackObserved,
                 successorCursor, restartRequired, failureReason,
                 tornMidpointOpenRejected, validationHistoryVars>>

IgnoreForeignOwnerCandidateAndRetainLocalReceipt ==
  /\ receiptStage = "Minted"
  /\ ExactPredecessorReceipt
  /\ ExactSuccessorConstruction
  /\ foreignReceiptCandidatePresent
  /\ retainedSuccessor = NoIdentity
  /\ ~successorActive
  /\ ~restartRequired
  /\ receiptStage' = "Retained"
  /\ retainedSuccessor' = ExpectedSuccessor
  /\ foreignReceiptCandidatePresent' = FALSE
  /\ UNCHANGED <<targetRoster, finalityValidated, workerIngressClosed,
                 workerOutstanding, ownerSealed, constructionParent,
                 constructionSuccessor, receiptOwnerNonce, receiptContext,
                 receiptArtifact, receiptConsumeCount, serviceGeneration,
                 durableHighWater, responderActive, responderWritable,
                 responderAuthorized, retryableChunk, successorActive,
                 transitionAuthority, forcedTransitionUsed,
                 transitionedFromLiveResponder,
                 ordinaryLiveTransitionRejected, foreignReceiptRejected,
                 lateEnqueueRejected, lateOldCallbackObserved,
                 successorCursor, restartRequired, failureReason,
                 tornMidpointOpenRejected,
                 validationHistoryVars>>

CleanRejectSameContextForeignOwnerReceipt ==
  /\ foreignReceiptCandidatePresent
  /\ ForeignOwnerNonce # TransportOwnerNonce
  /\ ~foreignReceiptRejected
  /\ ~restartRequired
  /\ foreignReceiptRejected' = TRUE
  /\ foreignReceiptCandidatePresent' = FALSE
  /\ UNCHANGED <<targetRoster, finalityValidated, workerIngressClosed,
                 workerOutstanding, ownerSealed, constructionParent,
                 constructionSuccessor, receiptStage, receiptOwnerNonce,
                 receiptContext, receiptArtifact, retainedSuccessor,
                 receiptConsumeCount, serviceGeneration, durableHighWater,
                 responderActive, responderWritable, responderAuthorized,
                 retryableChunk, successorActive, transitionAuthority,
                 forcedTransitionUsed, transitionedFromLiveResponder,
                 ordinaryLiveTransitionRejected, lateEnqueueRejected,
                 lateOldCallbackObserved, successorCursor, restartRequired,
                 failureReason, tornMidpointOpenRejected,
                 validationHistoryVars>>

AcceptMismatchedPredecessorReceipt(kind) ==
  /\ kind \in {"ContextMismatch", "ArtifactMismatch"}
  /\ receiptStage = "Minted"
  /\ ExactPredecessorReceipt
  /\ ExactSuccessorConstruction
  /\ predecessorMismatchCandidateKind = kind
  /\ retainedSuccessor = NoIdentity
  /\ ~successorActive
  /\ ~restartRequired
  /\ receiptStage' = "Retained"
  /\ receiptContext' =
       IF kind = "ContextMismatch" THEN "ContextB" ELSE receiptContext
  /\ receiptArtifact' =
       IF kind = "ArtifactMismatch" THEN "ArtifactB" ELSE receiptArtifact
  /\ retainedSuccessor' = ExpectedSuccessor
  /\ predecessorMismatchCandidateKind' = "NoMismatch"
  /\ UNCHANGED <<targetRoster, finalityValidated, workerIngressClosed,
                 workerOutstanding, ownerSealed, constructionParent,
                 constructionSuccessor, receiptOwnerNonce,
                 receiptConsumeCount, serviceGeneration, durableHighWater,
                 responderActive, responderWritable, responderAuthorized,
                 retryableChunk, successorActive, transitionAuthority,
                 forcedTransitionUsed, transitionedFromLiveResponder,
                 ordinaryLiveTransitionRejected, foreignReceiptRejected,
                 lateEnqueueRejected, lateOldCallbackObserved,
                 successorCursor, restartRequired, failureReason,
                 tornMidpointOpenRejected, foreignReceiptCandidatePresent,
                 validationHistoriesExceptPredecessorCandidate>>

CleanRejectMismatchedPredecessorReceipt(kind) ==
  /\ kind \in {"ContextMismatch", "ArtifactMismatch"}
  /\ predecessorMismatchCandidateKind = kind
  /\ ~predecessorMismatchRejected
  /\ ~restartRequired
  /\ predecessorMismatchCandidateKind' = "NoMismatch"
  /\ predecessorMismatchRejected' = TRUE
  /\ UNCHANGED <<targetRoster, finalityValidated, workerIngressClosed,
                 workerOutstanding, ownerSealed, constructionParent,
                 constructionSuccessor, receiptStage, receiptOwnerNonce,
                 receiptContext, receiptArtifact, retainedSuccessor,
                 receiptConsumeCount, serviceGeneration, durableHighWater,
                 responderActive, responderWritable, responderAuthorized,
                 retryableChunk, successorActive, transitionAuthority,
                 forcedTransitionUsed, transitionedFromLiveResponder,
                 ordinaryLiveTransitionRejected, foreignReceiptRejected,
                 lateEnqueueRejected, lateOldCallbackObserved,
                 successorCursor, restartRequired, failureReason,
                 tornMidpointOpenRejected, foreignReceiptCandidatePresent,
                 tornHighWaterHistory, foreignReceiptCandidateObserved,
                 predecessorMismatchObservedKind,
                 wrongSuccessorCandidatePresent,
                 wrongSuccessorCandidateObserved, wrongSuccessorRejected>>

RetainForeignSuccessorIdentity ==
  /\ receiptStage = "Minted"
  /\ ExactPredecessorReceipt
  /\ ExactSuccessorConstruction
  /\ wrongSuccessorCandidatePresent
  /\ retainedSuccessor = NoIdentity
  /\ ~successorActive
  /\ ~restartRequired
  /\ receiptStage' = "Retained"
  /\ retainedSuccessor' = ForeignSuccessor
  /\ wrongSuccessorCandidatePresent' = FALSE
  /\ UNCHANGED <<targetRoster, finalityValidated, workerIngressClosed,
                 workerOutstanding, ownerSealed, constructionParent,
                 constructionSuccessor, receiptOwnerNonce, receiptContext,
                 receiptArtifact, receiptConsumeCount, serviceGeneration,
                 durableHighWater, responderActive, responderWritable,
                 responderAuthorized, retryableChunk, successorActive,
                 transitionAuthority, forcedTransitionUsed,
                 transitionedFromLiveResponder,
                 ordinaryLiveTransitionRejected, foreignReceiptRejected,
                 lateEnqueueRejected, lateOldCallbackObserved,
                 successorCursor, restartRequired, failureReason,
                 tornMidpointOpenRejected,
                 foreignReceiptCandidatePresent,
                 validationHistoriesExceptWrongSuccessorCandidate>>

CleanRejectWrongImmediateSuccessor ==
  /\ wrongSuccessorCandidatePresent
  /\ ~wrongSuccessorRejected
  /\ ~restartRequired
  /\ wrongSuccessorCandidatePresent' = FALSE
  /\ wrongSuccessorRejected' = TRUE
  /\ UNCHANGED <<targetRoster, finalityValidated, workerIngressClosed,
                 workerOutstanding, ownerSealed, constructionParent,
                 constructionSuccessor, receiptStage, receiptOwnerNonce,
                 receiptContext, receiptArtifact, retainedSuccessor,
                 receiptConsumeCount, serviceGeneration, durableHighWater,
                 responderActive, responderWritable, responderAuthorized,
                 retryableChunk, successorActive, transitionAuthority,
                 forcedTransitionUsed, transitionedFromLiveResponder,
                 ordinaryLiveTransitionRejected, foreignReceiptRejected,
                 lateEnqueueRejected, lateOldCallbackObserved,
                 successorCursor, restartRequired, failureReason,
                 tornMidpointOpenRejected, foreignReceiptCandidatePresent,
                 tornHighWaterHistory, foreignReceiptCandidateObserved,
                 predecessorMismatchCandidateKind,
                 predecessorMismatchObservedKind,
                 predecessorMismatchRejected,
                 wrongSuccessorCandidateObserved>>

EnqueueAfterOwnerSeal ==
  /\ ownerSealed
  /\ workerOutstanding = 0
  /\ ~restartRequired
  /\ workerOutstanding' = 1
  /\ UNCHANGED <<targetRoster, finalityValidated, workerIngressClosed,
                 ownerSealed, constructionParent, constructionSuccessor,
                 receiptStage, receiptOwnerNonce, receiptContext,
                 receiptArtifact, retainedSuccessor, receiptConsumeCount,
                 serviceGeneration, durableHighWater, responderActive,
                 responderWritable, responderAuthorized, retryableChunk,
                 successorActive, transitionAuthority, forcedTransitionUsed,
                 transitionedFromLiveResponder,
                 ordinaryLiveTransitionRejected, foreignReceiptRejected,
                 lateEnqueueRejected, lateOldCallbackObserved,
                 successorCursor, restartRequired, failureReason,
                 tornMidpointOpenRejected,
                 foreignReceiptCandidatePresent,
                 validationHistoryVars>>

CleanRejectLateExactOutputEnqueue ==
  /\ ownerSealed
  /\ receiptStage = "Minted"
  /\ ~successorActive
  /\ ~restartRequired
  /\ ~ValidationCandidatePending
  /\ ~lateEnqueueRejected
  /\ lateEnqueueRejected' = TRUE
  /\ UNCHANGED <<targetRoster, finalityValidated, workerIngressClosed,
                 workerOutstanding, ownerSealed, constructionParent,
                 constructionSuccessor, receiptStage, receiptOwnerNonce,
                 receiptContext, receiptArtifact, retainedSuccessor,
                 receiptConsumeCount, serviceGeneration, durableHighWater,
                 responderActive, responderWritable, responderAuthorized,
                 retryableChunk, successorActive, transitionAuthority,
                 forcedTransitionUsed, transitionedFromLiveResponder,
                 ordinaryLiveTransitionRejected, foreignReceiptRejected,
                 lateOldCallbackObserved, successorCursor, restartRequired,
                 failureReason, tornMidpointOpenRejected,
                 foreignReceiptCandidatePresent,
                 validationHistoryVars>>

CleanFailNextServiceHighWaterPersistence ==
  /\ targetRoster = "ChangedRoster"
  /\ ExactRetainedMergeSidecars
  /\ durableHighWater = InitialGeneration
  /\ serviceGeneration = InitialGeneration
  /\ failureReason = "None"
  /\ ~restartRequired
  /\ ~successorActive
  /\ failureReason' = "PersistenceFailure"
  /\ UNCHANGED <<targetRoster, finalityValidated, workerIngressClosed,
                 workerOutstanding, ownerSealed, constructionParent,
                 constructionSuccessor, receiptStage, receiptOwnerNonce,
                 receiptContext, receiptArtifact, retainedSuccessor,
                 receiptConsumeCount, serviceGeneration, durableHighWater,
                 responderActive, responderWritable, responderAuthorized,
                 retryableChunk, successorActive, transitionAuthority,
                 forcedTransitionUsed, transitionedFromLiveResponder,
                 ordinaryLiveTransitionRejected, foreignReceiptRejected,
                 lateEnqueueRejected, lateOldCallbackObserved,
                 successorCursor, restartRequired, tornMidpointOpenRejected,
                 foreignReceiptCandidatePresent, validationHistoryVars>>

CleanFailLifecycleSnapshotAfterHighWaterPersistence ==
  /\ targetRoster = "ChangedRoster"
  /\ ExactRetainedMergeSidecars
  /\ PreparedHighWaterSnapshotMidpoint
  /\ ~successorActive
  /\ failureReason' = "SnapshotPersistenceFailure"
  /\ tornHighWaterHistory' = TRUE
  /\ UNCHANGED <<targetRoster, finalityValidated, workerIngressClosed,
                 workerOutstanding, ownerSealed, constructionParent,
                 constructionSuccessor, receiptStage, receiptOwnerNonce,
                 receiptContext, receiptArtifact, retainedSuccessor,
                 receiptConsumeCount, serviceGeneration, durableHighWater,
                 responderActive, responderWritable, responderAuthorized,
                 retryableChunk, successorActive, transitionAuthority,
                 forcedTransitionUsed, transitionedFromLiveResponder,
                 ordinaryLiveTransitionRejected, foreignReceiptRejected,
                 lateEnqueueRejected, lateOldCallbackObserved,
                 successorCursor, restartRequired, tornMidpointOpenRejected,
                 foreignReceiptCandidatePresent,
                 validationHistoriesExceptTorn>>

FailLifecycleSnapshotWithoutTornHistory ==
  /\ targetRoster = "ChangedRoster"
  /\ ExactRetainedMergeSidecars
  /\ PreparedHighWaterSnapshotMidpoint
  /\ ~successorActive
  /\ restartRequired' = TRUE
  /\ successorActive' = FALSE
  /\ failureReason' = "SnapshotPersistenceFailure"
  /\ UNCHANGED <<targetRoster, finalityValidated, workerIngressClosed,
                 workerOutstanding, ownerSealed, constructionParent,
                 constructionSuccessor, receiptStage, receiptOwnerNonce,
                 receiptContext, receiptArtifact, retainedSuccessor,
                 receiptConsumeCount, serviceGeneration, durableHighWater,
                 responderActive, responderWritable, responderAuthorized,
                 retryableChunk, transitionAuthority, forcedTransitionUsed,
                 transitionedFromLiveResponder,
                 ordinaryLiveTransitionRejected, foreignReceiptRejected,
                 lateEnqueueRejected, lateOldCallbackObserved,
                 successorCursor, tornMidpointOpenRejected,
                 foreignReceiptCandidatePresent, validationHistoryVars>>

UntypedLiveChangedRosterForce ==
  /\ targetRoster = "ChangedRoster"
  /\ ExactRetainedMergeSidecars
  /\ ~successorActive
  /\ ResponderBlocksOrdinaryTransition
  /\ ~restartRequired
  /\ serviceGeneration' = NextGeneration
  /\ durableHighWater' = NextGeneration
  /\ responderActive' = FALSE
  /\ responderWritable' = FALSE
  /\ responderAuthorized' = FALSE
  /\ retryableChunk' = 0
  /\ successorActive' = TRUE
  /\ transitionAuthority' = "Ordinary"
  /\ forcedTransitionUsed' = TRUE
  /\ transitionedFromLiveResponder' = TRUE
  /\ UNCHANGED <<targetRoster, finalityValidated, workerIngressClosed,
                 workerOutstanding, ownerSealed, constructionParent,
                 constructionSuccessor, receiptStage, receiptOwnerNonce,
                 receiptContext, receiptArtifact, retainedSuccessor,
                 receiptConsumeCount, ordinaryLiveTransitionRejected,
                 foreignReceiptRejected, lateEnqueueRejected,
                 lateOldCallbackObserved, successorCursor, restartRequired,
                 failureReason, tornMidpointOpenRejected,
                 foreignReceiptCandidatePresent,
                 validationHistoryVars>>

ActivateBeforeHighWaterPersistence ==
  /\ targetRoster = "ChangedRoster"
  /\ ExactRetainedMergeSidecars
  /\ durableHighWater = InitialGeneration
  /\ ~successorActive
  /\ ~restartRequired
  /\ receiptStage' = "Consumed"
  /\ receiptConsumeCount' = 1
  /\ serviceGeneration' = NextGeneration
  /\ responderActive' = FALSE
  /\ responderWritable' = FALSE
  /\ responderAuthorized' = FALSE
  /\ retryableChunk' = 0
  /\ successorActive' = TRUE
  /\ transitionAuthority' = "Typed"
  /\ forcedTransitionUsed' = TRUE
  /\ transitionedFromLiveResponder' = ResponderBlocksOrdinaryTransition
  /\ UNCHANGED <<targetRoster, finalityValidated, workerIngressClosed,
                 workerOutstanding, ownerSealed, constructionParent,
                 constructionSuccessor, receiptOwnerNonce, receiptContext,
                 receiptArtifact, retainedSuccessor, durableHighWater,
                 ordinaryLiveTransitionRejected, foreignReceiptRejected,
                 lateEnqueueRejected, lateOldCallbackObserved,
                 successorCursor, restartRequired, failureReason,
                 tornMidpointOpenRejected,
                 foreignReceiptCandidatePresent,
                 validationHistoryVars>>

SameRosterDropsRetryableChunk ==
  /\ targetRoster = "SameRoster"
  /\ ExactRetainedMergeSidecars
  /\ durableHighWater = InitialGeneration
  /\ ~successorActive
  /\ ~restartRequired
  /\ receiptStage' = "Consumed"
  /\ receiptConsumeCount' = 1
  /\ retryableChunk' = 0
  /\ successorActive' = TRUE
  /\ transitionAuthority' = "RetainedSameRoster"
  /\ forcedTransitionUsed' = FALSE
  /\ transitionedFromLiveResponder' = ResponderBlocksOrdinaryTransition
  /\ UNCHANGED <<targetRoster, finalityValidated, workerIngressClosed,
                 workerOutstanding, ownerSealed, constructionParent,
                 constructionSuccessor, receiptOwnerNonce, receiptContext,
                 receiptArtifact, retainedSuccessor, serviceGeneration,
                 durableHighWater, responderActive, responderWritable,
                 responderAuthorized, ordinaryLiveTransitionRejected,
                 foreignReceiptRejected, lateEnqueueRejected,
                 lateOldCallbackObserved, successorCursor, restartRequired,
                 failureReason, tornMidpointOpenRejected,
                 foreignReceiptCandidatePresent,
                 validationHistoryVars>>

LateOldWriterMutatesSuccessor ==
  /\ targetRoster = "ChangedRoster"
  /\ successorActive
  /\ transitionAuthority = "Typed"
  /\ ~lateOldCallbackObserved
  /\ lateOldCallbackObserved' = TRUE
  /\ successorCursor' = 1
  /\ UNCHANGED <<targetRoster, finalityValidated, workerIngressClosed,
                 workerOutstanding, ownerSealed, constructionParent,
                 constructionSuccessor, receiptStage, receiptOwnerNonce,
                 receiptContext, receiptArtifact, retainedSuccessor,
                 receiptConsumeCount, serviceGeneration, durableHighWater,
                 responderActive, responderWritable, responderAuthorized,
                 retryableChunk, successorActive, transitionAuthority,
                 forcedTransitionUsed, transitionedFromLiveResponder,
                 ordinaryLiveTransitionRejected, foreignReceiptRejected,
                 lateEnqueueRejected, restartRequired, failureReason,
                 tornMidpointOpenRejected,
                 foreignReceiptCandidatePresent,
                 validationHistoryVars>>

OpenTornHighWaterAheadSnapshot ==
  /\ targetRoster = "ChangedRoster"
  /\ ExactRetainedMergeSidecars
  /\ TornHighWaterSnapshotMidpoint
  /\ tornHighWaterHistory
  /\ ~successorActive
  /\ receiptStage' = "Consumed"
  /\ receiptConsumeCount' = 1
  /\ serviceGeneration' = NextGeneration
  /\ responderActive' = FALSE
  /\ responderWritable' = FALSE
  /\ responderAuthorized' = FALSE
  /\ retryableChunk' = 0
  /\ successorActive' = TRUE
  /\ transitionAuthority' = "Typed"
  /\ forcedTransitionUsed' = TRUE
  /\ transitionedFromLiveResponder' = ResponderBlocksOrdinaryTransition
  /\ restartRequired' = FALSE
  /\ failureReason' = "None"
  /\ UNCHANGED <<targetRoster, finalityValidated, workerIngressClosed,
                 workerOutstanding, ownerSealed, constructionParent,
                 constructionSuccessor, receiptOwnerNonce, receiptContext,
                 receiptArtifact, retainedSuccessor, durableHighWater,
                 ordinaryLiveTransitionRejected,
                 foreignReceiptRejected, lateEnqueueRejected,
                 lateOldCallbackObserved, successorCursor,
                 tornMidpointOpenRejected,
                 foreignReceiptCandidatePresent,
                 validationHistoryVars>>

MutationSeal ==
  IF MutationMode = "PrematureSeal"
  THEN PrematureSealAndMint
  ELSE SealAppliedHeightOutputHandoff

MutationPresentValidationCandidate ==
  CASE MutationMode \in
         {"ForeignOwnerNonce",
          "IgnoreForeignCandidate",
          "CleanForeignOwnerReject"} ->
         PresentSameContextForeignOwnerReceipt
    [] MutationMode \in
         {"AcceptPredecessorContextMismatch",
          "CleanPredecessorContextReject"} ->
         PresentMismatchedPredecessorContextReceipt
    [] MutationMode \in
         {"AcceptPredecessorArtifactMismatch",
          "CleanPredecessorArtifactReject"} ->
         PresentMismatchedPredecessorArtifactReceipt
    [] MutationMode \in
         {"ForeignSuccessor", "CleanWrongSuccessorReject"} ->
         PresentWrongImmediateSuccessor
    [] OTHER ->
         \/ PresentSameContextForeignOwnerReceipt
         \/ PresentMismatchedPredecessorContextReceipt
         \/ PresentMismatchedPredecessorArtifactReceipt
         \/ PresentWrongImmediateSuccessor

MutationForeignOwnerReject ==
  IF MutationMode = "CleanForeignOwnerReject"
  THEN CleanRejectSameContextForeignOwnerReceipt
  ELSE RejectSameContextForeignOwnerReceipt

MutationPredecessorMismatchReject ==
  CASE MutationMode = "CleanPredecessorContextReject" ->
         CleanRejectMismatchedPredecessorReceipt("ContextMismatch")
    [] MutationMode = "CleanPredecessorArtifactReject" ->
         CleanRejectMismatchedPredecessorReceipt("ArtifactMismatch")
    [] OTHER ->
         RejectMismatchedPredecessorReceipt

MutationWrongSuccessorReject ==
  IF MutationMode = "CleanWrongSuccessorReject"
  THEN CleanRejectWrongImmediateSuccessor
  ELSE RejectWrongImmediateSuccessor

MutationRetain ==
  CASE MutationMode = "ForeignOwnerNonce" ->
         RetainSameContextForeignOwnerReceipt
    [] MutationMode = "IgnoreForeignCandidate" ->
         IgnoreForeignOwnerCandidateAndRetainLocalReceipt
    [] MutationMode = "AcceptPredecessorContextMismatch" ->
         AcceptMismatchedPredecessorReceipt("ContextMismatch")
    [] MutationMode = "AcceptPredecessorArtifactMismatch" ->
         AcceptMismatchedPredecessorReceipt("ArtifactMismatch")
    [] MutationMode = "ForeignSuccessor" ->
         RetainForeignSuccessorIdentity
    [] OTHER ->
         ConsumeReceiptIntoRetainedMergeSidecars

MutationLateEnqueue ==
  CASE MutationMode = "LateEnqueue" ->
         EnqueueAfterOwnerSeal
    [] MutationMode = "CleanLateEnqueueReject" ->
         CleanRejectLateExactOutputEnqueue
    [] OTHER ->
         RejectLateExactOutputEnqueue

MutationChangedRosterTransition ==
  CASE MutationMode = "UntypedForce" ->
         UntypedLiveChangedRosterForce
    [] MutationMode = "SkipHighWater" ->
         ActivateBeforeHighWaterPersistence
    [] OTHER ->
         TypedChangedRosterTransition

MutationSameRosterTransition ==
  IF MutationMode = "LoseSameRosterRetry"
  THEN SameRosterDropsRetryableChunk
  ELSE SameRosterRetainedTransportRollover

MutationLateCallback ==
  IF MutationMode = "LateOldCallback"
  THEN LateOldWriterMutatesSuccessor
  ELSE ObserveLateOldWriterCallback

MutationTornMidpointOpen ==
  IF MutationMode = "OpenHighWaterAheadSnapshot"
  THEN OpenTornHighWaterAheadSnapshot
  ELSE RejectTornHighWaterSnapshotOpen

MutationHighWaterPersistenceFailure ==
  IF MutationMode = "CleanHighWaterPersistenceFailure"
  THEN CleanFailNextServiceHighWaterPersistence
  ELSE FailNextServiceHighWaterPersistence

MutationLifecycleSnapshotPersistenceFailure ==
  CASE MutationMode = "CleanLifecycleSnapshotPersistenceFailure" ->
         CleanFailLifecycleSnapshotAfterHighWaterPersistence
    [] MutationMode = "OmitLifecycleSnapshotTornHistory" ->
         FailLifecycleSnapshotWithoutTornHistory
    [] OTHER ->
         FailLifecycleSnapshotAfterHighWaterPersistence

MutationNext ==
  \/ ValidateFinality
  \/ CloseWorkerIngress
  \/ ClearOneWorkerExactOutput
  \/ MutationSeal
  \/ MutationLateEnqueue
  \/ BeginExactSuccessorConstruction
  \/ MutationPresentValidationCandidate
  \/ MutationForeignOwnerReject
  \/ MutationPredecessorMismatchReject
  \/ MutationWrongSuccessorReject
  \/ MutationRetain
  \/ PersistNextServiceHighWater
  \/ MutationHighWaterPersistenceFailure
  \/ MutationLifecycleSnapshotPersistenceFailure
  \/ CrashAtHighWaterAheadSnapshot
  \/ MutationTornMidpointOpen
  \/ RejectOrdinaryLiveChangedRosterTransition
  \/ QuiesceChangedRosterResponder
  \/ MutationChangedRosterTransition
  \/ MutationSameRosterTransition
  \/ MutationLateCallback
  \/ CrashWithRolledBackHighWater

MutationSpec ==
  /\ MutationInit
  /\ [][MutationNext]_typedRolloverVars

=============================================================================
