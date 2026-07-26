---- MODULE SumeragiV2TypedRolloverHandoff ----
EXTENDS Naturals, TLC

(***************************************************************************
Orthogonal executable model of the move-only durable exact-output handoff.

The vocabulary mirrors the production Rust ownership chain:

  DurableExactOutputServiceOwner
    --seal_applied_height_output_handoff-->
  DurableExactOutputHandoffReceipt
    --exact DurableExactOutputTransportOwner + successor checks-->
  RetainedMergeSidecars
    --consume-->
  DurableMergeSidecarRolloverAuthority.

The service and transport endpoints share one private process-local owner
nonce. The final seal holds the corridor boundary, observes no remaining
exact-output ownership, atomically seals all later enqueue operations, and
mints the receipt. The retained wrapper additionally binds the exact finalized
predecessor artifact and immediate successor identity.

Late enqueue and foreign-owner consume attempts are unfinished guard
operations in the production implementation, so their rejection latches
restart and leaves no successor active. The same fail-stop rule is modeled
for predecessor context/artifact mismatches and wrong immediate-successor
inputs. Duplicate reseal misuse is outside this one-shot abstraction;
production treats that misuse as fail-stop too. Pending lane-output and
effect rejection belong to their dedicated ownership models and are outside
this leaf.

An equal roster consumes the authority only after preserving and requeueing
the immutable current chunk. A changed roster may consume it to advance the
durable service generation and atomically clear even an active+writable old
responder. The ordinary API has no such permission and rejects live or
authorized output.

Persistence is explicitly high-water first. Marker generation 2 with lifecycle
snapshot generation 1 is a valid prepared midpoint. Snapshot failure or crash
from that midpoint latches restart, and recovery/open can only reject it;
activation requires the lifecycle snapshot to catch up to the marker.

`ResponsiveTypedRolloverSpec` is only a local conditional liveness contract.
It assumes no local persistence or protocol-validation failure and weak
fairness for finite corridor clearing, successor construction, receipt
wrapping, foreign-candidate validation, and successor activation. It assumes
no network-writer flush, peer delivery, route-quiescence, or other unbounded
network fairness.
***************************************************************************)

NoIdentity == "NoIdentity"

RosterRelations == {"SameRoster", "ChangedRoster"}
ReceiptStages == {"Absent", "Minted", "Retained", "Consumed"}
TransitionAuthorities == {"None", "Ordinary", "Typed", "RetainedSameRoster"}
FailureReasons ==
  {"None",
   "PersistenceFailure",
   "SnapshotPersistenceFailure",
   "TornHighWaterSnapshot",
   "RollbackMismatch",
   "ForeignOwnerMismatch",
   "PredecessorContextMismatch",
   "PredecessorArtifactMismatch",
   "ImmediateSuccessorMismatch",
   "LateExactOutputEnqueue"}

OwnerNonces == {"OwnerNonce", "ForeignNonce"}
Contexts == {"ContextA", "ContextB"}
Artifacts == {"ArtifactA", "ArtifactB"}
Parents == {"ParentA", "ParentB"}
Successors == {"SuccessorA", "SuccessorB"}
PredecessorMismatchKinds ==
  {"NoMismatch", "ContextMismatch", "ArtifactMismatch"}

ServiceOwnerNonce == "OwnerNonce"
TransportOwnerNonce == "OwnerNonce"
ForeignOwnerNonce == "ForeignNonce"
ExpectedContext == "ContextA"
ExpectedArtifact == "ArtifactA"
ExpectedParent == "ParentA"
ExpectedSuccessor == "SuccessorA"
ForeignSuccessor == "SuccessorB"

InitialWorkerOutstanding == 2
InitialGeneration == 1
NextGeneration == 2

VARIABLES
  targetRoster,
  finalityValidated,
  workerIngressClosed,
  workerOutstanding,
  ownerSealed,
  constructionParent,
  constructionSuccessor,
  receiptStage,
  receiptOwnerNonce,
  receiptContext,
  receiptArtifact,
  retainedSuccessor,
  receiptConsumeCount,
  serviceGeneration,
  durableHighWater,
  responderActive,
  responderWritable,
  responderAuthorized,
  retryableChunk,
  successorActive,
  transitionAuthority,
  forcedTransitionUsed,
  transitionedFromLiveResponder,
  ordinaryLiveTransitionRejected,
  foreignReceiptRejected,
  lateEnqueueRejected,
  lateOldCallbackObserved,
  successorCursor,
  restartRequired,
  failureReason,
  tornMidpointOpenRejected,
  foreignReceiptCandidatePresent,
  tornHighWaterHistory,
  foreignReceiptCandidateObserved,
  predecessorMismatchCandidateKind,
  predecessorMismatchObservedKind,
  predecessorMismatchRejected,
  wrongSuccessorCandidatePresent,
  wrongSuccessorCandidateObserved,
  wrongSuccessorRejected

typedRolloverVars ==
  <<targetRoster, finalityValidated, workerIngressClosed,
    workerOutstanding, ownerSealed, constructionParent,
    constructionSuccessor, receiptStage, receiptOwnerNonce, receiptContext,
    receiptArtifact, retainedSuccessor, receiptConsumeCount,
    serviceGeneration, durableHighWater, responderActive,
    responderWritable, responderAuthorized, retryableChunk, successorActive,
    transitionAuthority, forcedTransitionUsed,
    transitionedFromLiveResponder, ordinaryLiveTransitionRejected,
    foreignReceiptRejected, lateEnqueueRejected, lateOldCallbackObserved,
    successorCursor, restartRequired, failureReason,
    tornMidpointOpenRejected, foreignReceiptCandidatePresent,
    tornHighWaterHistory, foreignReceiptCandidateObserved,
    predecessorMismatchCandidateKind, predecessorMismatchObservedKind,
    predecessorMismatchRejected, wrongSuccessorCandidatePresent,
    wrongSuccessorCandidateObserved, wrongSuccessorRejected>>

validationHistoryVars ==
  <<tornHighWaterHistory, foreignReceiptCandidateObserved,
    predecessorMismatchCandidateKind, predecessorMismatchObservedKind,
    predecessorMismatchRejected, wrongSuccessorCandidatePresent,
    wrongSuccessorCandidateObserved, wrongSuccessorRejected>>

validationHistoriesExceptTorn ==
  <<foreignReceiptCandidateObserved, predecessorMismatchCandidateKind,
    predecessorMismatchObservedKind, predecessorMismatchRejected,
    wrongSuccessorCandidatePresent, wrongSuccessorCandidateObserved,
    wrongSuccessorRejected>>

validationHistoriesExceptForeignOwner ==
  <<tornHighWaterHistory, predecessorMismatchCandidateKind,
    predecessorMismatchObservedKind, predecessorMismatchRejected,
    wrongSuccessorCandidatePresent, wrongSuccessorCandidateObserved,
    wrongSuccessorRejected>>

validationHistoriesExceptPredecessorCandidate ==
  <<tornHighWaterHistory, foreignReceiptCandidateObserved,
    predecessorMismatchObservedKind, predecessorMismatchRejected,
    wrongSuccessorCandidatePresent, wrongSuccessorCandidateObserved,
    wrongSuccessorRejected>>

validationHistoriesExceptWrongSuccessorCandidate ==
  <<tornHighWaterHistory, foreignReceiptCandidateObserved,
    predecessorMismatchCandidateKind, predecessorMismatchObservedKind,
    predecessorMismatchRejected, wrongSuccessorCandidateObserved,
    wrongSuccessorRejected>>

ResponderBlocksOrdinaryTransition ==
  \/ responderActive /\ responderWritable
  \/ responderAuthorized

ExactSuccessorConstruction ==
  /\ constructionParent = ExpectedParent
  /\ constructionSuccessor = ExpectedSuccessor

ExactServiceTransportOwnerPair ==
  /\ receiptOwnerNonce = ServiceOwnerNonce
  /\ receiptOwnerNonce = TransportOwnerNonce

ExactPredecessorReceipt ==
  /\ ExactServiceTransportOwnerPair
  /\ receiptContext = ExpectedContext
  /\ receiptArtifact = ExpectedArtifact

ExactRetainedMergeSidecars ==
  /\ receiptStage = "Retained"
  /\ ExactPredecessorReceipt
  /\ ExactSuccessorConstruction
  /\ retainedSuccessor = ExpectedSuccessor

FinalExactOutputSeal ==
  /\ finalityValidated
  /\ workerIngressClosed
  /\ workerOutstanding = 0
  /\ ownerSealed

HighWaterAheadOfLifecycleSnapshot ==
  /\ durableHighWater = NextGeneration
  /\ serviceGeneration = InitialGeneration

PreparedHighWaterSnapshotMidpoint ==
  /\ HighWaterAheadOfLifecycleSnapshot
  /\ ~restartRequired
  /\ failureReason = "None"

TornHighWaterSnapshotMidpoint ==
  /\ HighWaterAheadOfLifecycleSnapshot
  /\ restartRequired
  /\ failureReason \in
       {"SnapshotPersistenceFailure", "TornHighWaterSnapshot"}

PredecessorMismatchFailure(kind) ==
  IF kind = "ContextMismatch"
  THEN "PredecessorContextMismatch"
  ELSE "PredecessorArtifactMismatch"

ValidationCandidatePending ==
  \/ foreignReceiptCandidatePresent
  \/ predecessorMismatchCandidateKind # "NoMismatch"
  \/ wrongSuccessorCandidatePresent

Init ==
  /\ targetRoster \in RosterRelations
  /\ finalityValidated = FALSE
  /\ workerIngressClosed = FALSE
  /\ workerOutstanding = InitialWorkerOutstanding
  /\ ownerSealed = FALSE
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
  /\ responderActive = TRUE
  /\ responderWritable = TRUE
  /\ responderAuthorized = TRUE
  /\ retryableChunk = 1
  /\ successorActive = FALSE
  /\ transitionAuthority = "None"
  /\ forcedTransitionUsed = FALSE
  /\ transitionedFromLiveResponder = FALSE
  /\ ordinaryLiveTransitionRejected = FALSE
  /\ foreignReceiptRejected = FALSE
  /\ lateEnqueueRejected = FALSE
  /\ lateOldCallbackObserved = FALSE
  /\ successorCursor = 0
  /\ restartRequired = FALSE
  /\ failureReason = "None"
  /\ tornMidpointOpenRejected = FALSE
  /\ foreignReceiptCandidatePresent = FALSE
  /\ tornHighWaterHistory = FALSE
  /\ foreignReceiptCandidateObserved = FALSE
  /\ predecessorMismatchCandidateKind = "NoMismatch"
  /\ predecessorMismatchObservedKind = "NoMismatch"
  /\ predecessorMismatchRejected = FALSE
  /\ wrongSuccessorCandidatePresent = FALSE
  /\ wrongSuccessorCandidateObserved = FALSE
  /\ wrongSuccessorRejected = FALSE

ValidateFinality ==
  /\ ~finalityValidated
  /\ ~restartRequired
  /\ finalityValidated' = TRUE
  /\ UNCHANGED <<targetRoster, workerIngressClosed, workerOutstanding,
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

CloseWorkerIngress ==
  /\ finalityValidated
  /\ ~workerIngressClosed
  /\ ~restartRequired
  /\ workerIngressClosed' = TRUE
  /\ UNCHANGED <<targetRoster, finalityValidated, workerOutstanding,
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

ClearOneWorkerExactOutput ==
  /\ workerIngressClosed
  /\ workerOutstanding > 0
  /\ ~ownerSealed
  /\ ~restartRequired
  /\ workerOutstanding' = workerOutstanding - 1
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

SealAppliedHeightOutputHandoff ==
  /\ finalityValidated
  /\ workerIngressClosed
  /\ workerOutstanding = 0
  /\ ~ownerSealed
  /\ receiptStage = "Absent"
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

RejectLateExactOutputEnqueue ==
  /\ ownerSealed
  /\ receiptStage = "Minted"
  /\ ~successorActive
  /\ ~restartRequired
  /\ ~ValidationCandidatePending
  /\ ~lateEnqueueRejected
  /\ lateEnqueueRejected' = TRUE
  /\ successorActive' = FALSE
  /\ restartRequired' = TRUE
  /\ failureReason' = "LateExactOutputEnqueue"
  /\ UNCHANGED <<targetRoster, finalityValidated, workerIngressClosed,
                 workerOutstanding, ownerSealed, constructionParent,
                 constructionSuccessor, receiptStage, receiptOwnerNonce,
                 receiptContext, receiptArtifact, retainedSuccessor,
                 receiptConsumeCount, serviceGeneration, durableHighWater,
                 responderActive, responderWritable, responderAuthorized,
                 retryableChunk, transitionAuthority,
                 forcedTransitionUsed, transitionedFromLiveResponder,
                 ordinaryLiveTransitionRejected, foreignReceiptRejected,
                 lateOldCallbackObserved, successorCursor,
                 tornMidpointOpenRejected,
                 foreignReceiptCandidatePresent,
                 validationHistoryVars>>

BeginExactSuccessorConstruction ==
  /\ receiptStage = "Minted"
  /\ constructionParent = NoIdentity
  /\ constructionSuccessor = NoIdentity
  /\ ~restartRequired
  /\ constructionParent' = ExpectedParent
  /\ constructionSuccessor' = ExpectedSuccessor
  /\ UNCHANGED <<targetRoster, finalityValidated, workerIngressClosed,
                 workerOutstanding, ownerSealed, receiptStage,
                 receiptOwnerNonce, receiptContext, receiptArtifact,
                 retainedSuccessor, receiptConsumeCount, serviceGeneration,
                 durableHighWater, responderActive, responderWritable,
                 responderAuthorized, retryableChunk, successorActive,
                 transitionAuthority, forcedTransitionUsed,
                 transitionedFromLiveResponder,
                 ordinaryLiveTransitionRejected, foreignReceiptRejected,
                 lateEnqueueRejected, lateOldCallbackObserved,
                 successorCursor, restartRequired, failureReason,
                 tornMidpointOpenRejected,
                 foreignReceiptCandidatePresent,
                 validationHistoryVars>>

PresentSameContextForeignOwnerReceipt ==
  /\ receiptStage = "Minted"
  /\ ExactSuccessorConstruction
  /\ ~ValidationCandidatePending
  /\ ~foreignReceiptCandidateObserved
  /\ ~foreignReceiptRejected
  /\ ~successorActive
  /\ transitionAuthority = "None"
  /\ ~restartRequired
  /\ failureReason = "None"
  /\ foreignReceiptCandidatePresent' = TRUE
  /\ foreignReceiptCandidateObserved' = TRUE
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
                 tornMidpointOpenRejected,
                 validationHistoriesExceptForeignOwner>>

RejectSameContextForeignOwnerReceipt ==
  /\ foreignReceiptCandidatePresent
  /\ ForeignOwnerNonce # TransportOwnerNonce
  /\ ~foreignReceiptRejected
  /\ ~restartRequired
  /\ foreignReceiptRejected' = TRUE
  /\ foreignReceiptCandidatePresent' = FALSE
  /\ successorActive' = FALSE
  /\ restartRequired' = TRUE
  /\ failureReason' = "ForeignOwnerMismatch"
  /\ UNCHANGED <<targetRoster, finalityValidated, workerIngressClosed,
                 workerOutstanding, ownerSealed, constructionParent,
                 constructionSuccessor, receiptStage, receiptOwnerNonce,
                 receiptContext, receiptArtifact, retainedSuccessor,
                 receiptConsumeCount, serviceGeneration, durableHighWater,
                 responderActive, responderWritable, responderAuthorized,
                 retryableChunk, transitionAuthority,
                 forcedTransitionUsed, transitionedFromLiveResponder,
                 ordinaryLiveTransitionRejected, lateEnqueueRejected,
                 lateOldCallbackObserved, successorCursor,
                 tornMidpointOpenRejected, validationHistoryVars>>

PresentMismatchedPredecessorContextReceipt ==
  /\ receiptStage = "Minted"
  /\ ExactSuccessorConstruction
  /\ ~ValidationCandidatePending
  /\ predecessorMismatchObservedKind = "NoMismatch"
  /\ ~predecessorMismatchRejected
  /\ ~successorActive
  /\ transitionAuthority = "None"
  /\ ~restartRequired
  /\ failureReason = "None"
  /\ predecessorMismatchCandidateKind' = "ContextMismatch"
  /\ predecessorMismatchObservedKind' = "ContextMismatch"
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
                 predecessorMismatchRejected,
                 wrongSuccessorCandidatePresent,
                 wrongSuccessorCandidateObserved, wrongSuccessorRejected>>

PresentMismatchedPredecessorArtifactReceipt ==
  /\ receiptStage = "Minted"
  /\ ExactSuccessorConstruction
  /\ ~ValidationCandidatePending
  /\ predecessorMismatchObservedKind = "NoMismatch"
  /\ ~predecessorMismatchRejected
  /\ ~successorActive
  /\ transitionAuthority = "None"
  /\ ~restartRequired
  /\ failureReason = "None"
  /\ predecessorMismatchCandidateKind' = "ArtifactMismatch"
  /\ predecessorMismatchObservedKind' = "ArtifactMismatch"
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
                 predecessorMismatchRejected,
                 wrongSuccessorCandidatePresent,
                 wrongSuccessorCandidateObserved, wrongSuccessorRejected>>

RejectMismatchedPredecessorReceipt ==
  /\ predecessorMismatchCandidateKind \in
       {"ContextMismatch", "ArtifactMismatch"}
  /\ ~predecessorMismatchRejected
  /\ ~restartRequired
  /\ predecessorMismatchCandidateKind' = "NoMismatch"
  /\ predecessorMismatchRejected' = TRUE
  /\ successorActive' = FALSE
  /\ restartRequired' = TRUE
  /\ failureReason' =
       PredecessorMismatchFailure(predecessorMismatchCandidateKind)
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
                 foreignReceiptCandidatePresent, tornHighWaterHistory,
                 foreignReceiptCandidateObserved,
                 predecessorMismatchObservedKind,
                 wrongSuccessorCandidatePresent,
                 wrongSuccessorCandidateObserved, wrongSuccessorRejected>>

PresentWrongImmediateSuccessor ==
  /\ receiptStage = "Minted"
  /\ ExactSuccessorConstruction
  /\ ~ValidationCandidatePending
  /\ ~wrongSuccessorCandidateObserved
  /\ ~wrongSuccessorRejected
  /\ ~successorActive
  /\ transitionAuthority = "None"
  /\ ~restartRequired
  /\ failureReason = "None"
  /\ wrongSuccessorCandidatePresent' = TRUE
  /\ wrongSuccessorCandidateObserved' = TRUE
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
                 predecessorMismatchRejected, wrongSuccessorRejected>>

RejectWrongImmediateSuccessor ==
  /\ wrongSuccessorCandidatePresent
  /\ ~wrongSuccessorRejected
  /\ ~restartRequired
  /\ wrongSuccessorCandidatePresent' = FALSE
  /\ wrongSuccessorRejected' = TRUE
  /\ successorActive' = FALSE
  /\ restartRequired' = TRUE
  /\ failureReason' = "ImmediateSuccessorMismatch"
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
                 foreignReceiptCandidatePresent, tornHighWaterHistory,
                 foreignReceiptCandidateObserved,
                 predecessorMismatchCandidateKind,
                 predecessorMismatchObservedKind,
                 predecessorMismatchRejected,
                 wrongSuccessorCandidateObserved>>

ConsumeReceiptIntoRetainedMergeSidecars ==
  /\ receiptStage = "Minted"
  /\ ExactPredecessorReceipt
  /\ ExactSuccessorConstruction
  /\ retainedSuccessor = NoIdentity
  /\ ~successorActive
  /\ ~restartRequired
  /\ ~ValidationCandidatePending
  /\ receiptStage' = "Retained"
  /\ retainedSuccessor' = ExpectedSuccessor
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
                 validationHistoryVars>>

PersistNextServiceHighWater ==
  /\ targetRoster = "ChangedRoster"
  /\ ExactRetainedMergeSidecars
  /\ durableHighWater = InitialGeneration
  /\ serviceGeneration = InitialGeneration
  /\ failureReason = "None"
  /\ ~restartRequired
  /\ durableHighWater' = NextGeneration
  /\ UNCHANGED <<targetRoster, finalityValidated, workerIngressClosed,
                 workerOutstanding, ownerSealed, constructionParent,
                 constructionSuccessor, receiptStage, receiptOwnerNonce,
                 receiptContext, receiptArtifact, retainedSuccessor,
                 receiptConsumeCount, serviceGeneration, responderActive,
                 responderWritable, responderAuthorized, retryableChunk,
                 successorActive, transitionAuthority, forcedTransitionUsed,
                 transitionedFromLiveResponder,
                 ordinaryLiveTransitionRejected, foreignReceiptRejected,
                 lateEnqueueRejected, lateOldCallbackObserved,
                 successorCursor, restartRequired, failureReason,
                 tornMidpointOpenRejected,
                 foreignReceiptCandidatePresent,
                 validationHistoryVars>>

FailNextServiceHighWaterPersistence ==
  /\ targetRoster = "ChangedRoster"
  /\ ExactRetainedMergeSidecars
  /\ durableHighWater = InitialGeneration
  /\ serviceGeneration = InitialGeneration
  /\ failureReason = "None"
  /\ ~restartRequired
  /\ restartRequired' = TRUE
  /\ successorActive' = FALSE
  /\ failureReason' = "PersistenceFailure"
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
                 foreignReceiptCandidatePresent,
                 validationHistoryVars>>

FailLifecycleSnapshotAfterHighWaterPersistence ==
  /\ targetRoster = "ChangedRoster"
  /\ ExactRetainedMergeSidecars
  /\ PreparedHighWaterSnapshotMidpoint
  /\ ~successorActive
  /\ restartRequired' = TRUE
  /\ successorActive' = FALSE
  /\ failureReason' = "SnapshotPersistenceFailure"
  /\ tornHighWaterHistory' = TRUE
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
                 foreignReceiptCandidatePresent,
                 validationHistoriesExceptTorn>>

CrashAtHighWaterAheadSnapshot ==
  /\ targetRoster = "ChangedRoster"
  /\ ExactRetainedMergeSidecars
  /\ PreparedHighWaterSnapshotMidpoint
  /\ ~successorActive
  /\ restartRequired' = TRUE
  /\ successorActive' = FALSE
  /\ failureReason' = "TornHighWaterSnapshot"
  /\ tornHighWaterHistory' = TRUE
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
                 foreignReceiptCandidatePresent,
                 validationHistoriesExceptTorn>>

RejectTornHighWaterSnapshotOpen ==
  /\ TornHighWaterSnapshotMidpoint
  /\ tornHighWaterHistory
  /\ ~tornMidpointOpenRejected
  /\ tornMidpointOpenRejected' = TRUE
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
                 foreignReceiptCandidatePresent,
                 validationHistoryVars>>

RejectOrdinaryLiveChangedRosterTransition ==
  /\ targetRoster = "ChangedRoster"
  /\ ~successorActive
  /\ ResponderBlocksOrdinaryTransition
  /\ ~ordinaryLiveTransitionRejected
  /\ ordinaryLiveTransitionRejected' = TRUE
  /\ UNCHANGED <<targetRoster, finalityValidated, workerIngressClosed,
                 workerOutstanding, ownerSealed, constructionParent,
                 constructionSuccessor, receiptStage, receiptOwnerNonce,
                 receiptContext, receiptArtifact, retainedSuccessor,
                 receiptConsumeCount, serviceGeneration, durableHighWater,
                 responderActive, responderWritable, responderAuthorized,
                 retryableChunk, successorActive, transitionAuthority,
                 forcedTransitionUsed, transitionedFromLiveResponder,
                 foreignReceiptRejected, lateEnqueueRejected,
                 lateOldCallbackObserved, successorCursor, restartRequired,
                 failureReason, tornMidpointOpenRejected,
                 foreignReceiptCandidatePresent,
                 validationHistoryVars>>

QuiesceChangedRosterResponder ==
  /\ targetRoster = "ChangedRoster"
  /\ ~successorActive
  /\ ResponderBlocksOrdinaryTransition
  /\ ~restartRequired
  /\ responderActive' = FALSE
  /\ responderWritable' = FALSE
  /\ responderAuthorized' = FALSE
  /\ retryableChunk' = 0
  /\ UNCHANGED <<targetRoster, finalityValidated, workerIngressClosed,
                 workerOutstanding, ownerSealed, constructionParent,
                 constructionSuccessor, receiptStage, receiptOwnerNonce,
                 receiptContext, receiptArtifact, retainedSuccessor,
                 receiptConsumeCount, serviceGeneration, durableHighWater,
                 successorActive, transitionAuthority, forcedTransitionUsed,
                 transitionedFromLiveResponder,
                 ordinaryLiveTransitionRejected, foreignReceiptRejected,
                 lateEnqueueRejected, lateOldCallbackObserved,
                 successorCursor, restartRequired, failureReason,
                 tornMidpointOpenRejected,
                 foreignReceiptCandidatePresent,
                 validationHistoryVars>>

TypedChangedRosterTransition ==
  /\ targetRoster = "ChangedRoster"
  /\ ExactRetainedMergeSidecars
  /\ PreparedHighWaterSnapshotMidpoint
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

SameRosterRetainedTransportRollover ==
  /\ targetRoster = "SameRoster"
  /\ ExactRetainedMergeSidecars
  /\ durableHighWater = InitialGeneration
  /\ ~successorActive
  /\ ~restartRequired
  /\ receiptStage' = "Consumed"
  /\ receiptConsumeCount' = 1
  /\ successorActive' = TRUE
  /\ transitionAuthority' = "RetainedSameRoster"
  /\ forcedTransitionUsed' = FALSE
  /\ transitionedFromLiveResponder' = ResponderBlocksOrdinaryTransition
  /\ UNCHANGED <<targetRoster, finalityValidated, workerIngressClosed,
                 workerOutstanding, ownerSealed, constructionParent,
                 constructionSuccessor, receiptOwnerNonce, receiptContext,
                 receiptArtifact, retainedSuccessor, serviceGeneration,
                 durableHighWater, responderActive, responderWritable,
                 responderAuthorized, retryableChunk,
                 ordinaryLiveTransitionRejected, foreignReceiptRejected,
                 lateEnqueueRejected, lateOldCallbackObserved,
                 successorCursor, restartRequired, failureReason,
                 tornMidpointOpenRejected,
                 foreignReceiptCandidatePresent,
                 validationHistoryVars>>

ObserveLateOldWriterCallback ==
  /\ targetRoster = "ChangedRoster"
  /\ successorActive
  /\ transitionAuthority = "Typed"
  /\ ~lateOldCallbackObserved
  /\ lateOldCallbackObserved' = TRUE
  /\ UNCHANGED <<targetRoster, finalityValidated, workerIngressClosed,
                 workerOutstanding, ownerSealed, constructionParent,
                 constructionSuccessor, receiptStage, receiptOwnerNonce,
                 receiptContext, receiptArtifact, retainedSuccessor,
                 receiptConsumeCount, serviceGeneration, durableHighWater,
                 responderActive, responderWritable, responderAuthorized,
                 retryableChunk, successorActive, transitionAuthority,
                 forcedTransitionUsed, transitionedFromLiveResponder,
                 ordinaryLiveTransitionRejected, foreignReceiptRejected,
                 lateEnqueueRejected, successorCursor, restartRequired,
                 failureReason, tornMidpointOpenRejected,
                 foreignReceiptCandidatePresent,
                 validationHistoryVars>>

CrashWithRolledBackHighWater ==
  /\ serviceGeneration = NextGeneration
  /\ durableHighWater = NextGeneration
  /\ successorActive
  /\ ~restartRequired
  /\ durableHighWater' = InitialGeneration
  /\ successorActive' = FALSE
  /\ restartRequired' = TRUE
  /\ failureReason' = "RollbackMismatch"
  /\ UNCHANGED <<targetRoster, finalityValidated, workerIngressClosed,
                 workerOutstanding, ownerSealed, constructionParent,
                 constructionSuccessor, receiptStage, receiptOwnerNonce,
                 receiptContext, receiptArtifact, retainedSuccessor,
                 receiptConsumeCount, serviceGeneration, responderActive,
                 responderWritable, responderAuthorized, retryableChunk,
                 transitionAuthority, forcedTransitionUsed,
                 transitionedFromLiveResponder,
                 ordinaryLiveTransitionRejected, foreignReceiptRejected,
                 lateEnqueueRejected, lateOldCallbackObserved,
                 successorCursor, tornMidpointOpenRejected,
                 foreignReceiptCandidatePresent,
                 validationHistoryVars>>

Next ==
  \/ ValidateFinality
  \/ CloseWorkerIngress
  \/ ClearOneWorkerExactOutput
  \/ SealAppliedHeightOutputHandoff
  \/ RejectLateExactOutputEnqueue
  \/ BeginExactSuccessorConstruction
  \/ PresentSameContextForeignOwnerReceipt
  \/ RejectSameContextForeignOwnerReceipt
  \/ PresentMismatchedPredecessorContextReceipt
  \/ PresentMismatchedPredecessorArtifactReceipt
  \/ RejectMismatchedPredecessorReceipt
  \/ PresentWrongImmediateSuccessor
  \/ RejectWrongImmediateSuccessor
  \/ ConsumeReceiptIntoRetainedMergeSidecars
  \/ PersistNextServiceHighWater
  \/ FailNextServiceHighWaterPersistence
  \/ FailLifecycleSnapshotAfterHighWaterPersistence
  \/ CrashAtHighWaterAheadSnapshot
  \/ RejectTornHighWaterSnapshotOpen
  \/ RejectOrdinaryLiveChangedRosterTransition
  \/ QuiesceChangedRosterResponder
  \/ TypedChangedRosterTransition
  \/ SameRosterRetainedTransportRollover
  \/ ObserveLateOldWriterCallback
  \/ CrashWithRolledBackHighWater

TypedRolloverSpec ==
  /\ Init
  /\ [][Next]_typedRolloverVars

ResponsiveTypedRolloverSpec ==
  /\ TypedRolloverSpec
  /\ WF_typedRolloverVars(CloseWorkerIngress)
  /\ WF_typedRolloverVars(ClearOneWorkerExactOutput)
  /\ WF_typedRolloverVars(SealAppliedHeightOutputHandoff)
  /\ WF_typedRolloverVars(BeginExactSuccessorConstruction)
  /\ WF_typedRolloverVars(RejectSameContextForeignOwnerReceipt)
  /\ WF_typedRolloverVars(RejectMismatchedPredecessorReceipt)
  /\ WF_typedRolloverVars(RejectWrongImmediateSuccessor)
  /\ WF_typedRolloverVars(ConsumeReceiptIntoRetainedMergeSidecars)
  /\ WF_typedRolloverVars(PersistNextServiceHighWater)
  /\ WF_typedRolloverVars(TypedChangedRosterTransition)

TypedRolloverTypeInvariant ==
  /\ targetRoster \in RosterRelations
  /\ finalityValidated \in BOOLEAN
  /\ workerIngressClosed \in BOOLEAN
  /\ workerOutstanding \in 0..InitialWorkerOutstanding
  /\ ownerSealed \in BOOLEAN
  /\ constructionParent \in Parents \cup {NoIdentity}
  /\ constructionSuccessor \in Successors \cup {NoIdentity}
  /\ receiptStage \in ReceiptStages
  /\ receiptOwnerNonce \in OwnerNonces \cup {NoIdentity}
  /\ receiptContext \in Contexts \cup {NoIdentity}
  /\ receiptArtifact \in Artifacts \cup {NoIdentity}
  /\ retainedSuccessor \in Successors \cup {NoIdentity}
  /\ receiptConsumeCount \in 0..1
  /\ serviceGeneration \in InitialGeneration..NextGeneration
  /\ durableHighWater \in InitialGeneration..NextGeneration
  /\ responderActive \in BOOLEAN
  /\ responderWritable \in BOOLEAN
  /\ responderAuthorized \in BOOLEAN
  /\ retryableChunk \in 0..1
  /\ successorActive \in BOOLEAN
  /\ transitionAuthority \in TransitionAuthorities
  /\ forcedTransitionUsed \in BOOLEAN
  /\ transitionedFromLiveResponder \in BOOLEAN
  /\ ordinaryLiveTransitionRejected \in BOOLEAN
  /\ foreignReceiptRejected \in BOOLEAN
  /\ lateEnqueueRejected \in BOOLEAN
  /\ lateOldCallbackObserved \in BOOLEAN
  /\ successorCursor \in 0..1
  /\ restartRequired \in BOOLEAN
  /\ failureReason \in FailureReasons
  /\ tornMidpointOpenRejected \in BOOLEAN
  /\ foreignReceiptCandidatePresent \in BOOLEAN
  /\ tornHighWaterHistory \in BOOLEAN
  /\ foreignReceiptCandidateObserved \in BOOLEAN
  /\ predecessorMismatchCandidateKind \in PredecessorMismatchKinds
  /\ predecessorMismatchObservedKind \in PredecessorMismatchKinds
  /\ predecessorMismatchRejected \in BOOLEAN
  /\ wrongSuccessorCandidatePresent \in BOOLEAN
  /\ wrongSuccessorCandidateObserved \in BOOLEAN
  /\ wrongSuccessorRejected \in BOOLEAN

ReceiptLifecycleInvariant ==
  /\ (receiptStage = "Absent" =>
        /\ receiptConsumeCount = 0
        /\ receiptOwnerNonce = NoIdentity
        /\ receiptContext = NoIdentity
        /\ receiptArtifact = NoIdentity
        /\ retainedSuccessor = NoIdentity)
  /\ (receiptStage \in {"Minted", "Retained"} =>
        receiptConsumeCount = 0)
  /\ (receiptStage = "Consumed" => receiptConsumeCount = 1)

FinalSealRejectsLateEnqueueInvariant ==
  /\ (ownerSealed =>
        /\ finalityValidated
        /\ workerIngressClosed
        /\ workerOutstanding = 0)
  /\ (receiptStage # "Absent" => ownerSealed)
  /\ (lateEnqueueRejected =>
        /\ ownerSealed
        /\ receiptStage = "Minted"
        /\ restartRequired
        /\ ~successorActive
        /\ failureReason = "LateExactOutputEnqueue")

ReceiptExactOwnerAndPredecessorInvariant ==
  receiptStage # "Absent" => ExactPredecessorReceipt

RetainedWrapperIdentityInvariant ==
  receiptStage \in {"Retained", "Consumed"} =>
    /\ ExactSuccessorConstruction
    /\ retainedSuccessor = ExpectedSuccessor

TransitionAuthorityLifecycleInvariant ==
  /\ (successorActive => transitionAuthority # "None")
  /\ (transitionAuthority # "None" =>
        successorActive \/ restartRequired)

OrdinaryChangedRosterTransitionInvariant ==
  transitionAuthority = "Ordinary" =>
    /\ targetRoster = "ChangedRoster"
    /\ ~transitionedFromLiveResponder
    /\ ~forcedTransitionUsed

LiveChangedRosterNeedsTypedReceiptInvariant ==
  /\ targetRoster = "ChangedRoster"
  /\ successorActive
  /\ transitionedFromLiveResponder
  =>
    /\ transitionAuthority = "Typed"
    /\ receiptStage = "Consumed"
    /\ receiptConsumeCount = 1
    /\ ExactPredecessorReceipt
    /\ retainedSuccessor = ExpectedSuccessor

TypedTransitionAtomicClearInvariant ==
  transitionAuthority = "Typed" =>
    /\ targetRoster = "ChangedRoster"
    /\ serviceGeneration = NextGeneration
    /\ ~responderActive
    /\ ~responderWritable
    /\ ~responderAuthorized
    /\ retryableChunk = 0
    /\ receiptStage = "Consumed"
    /\ receiptConsumeCount = 1
    /\ forcedTransitionUsed
    /\ (~restartRequired =>
          /\ successorActive
          /\ durableHighWater = NextGeneration)

SameRosterTransportPreservationInvariant ==
  targetRoster = "SameRoster" =>
    /\ serviceGeneration = InitialGeneration
    /\ durableHighWater = InitialGeneration
    /\ responderActive
    /\ responderWritable
    /\ responderAuthorized
    /\ retryableChunk = 1
    /\ ~forcedTransitionUsed

SameRosterRetryPreservationInvariant ==
  /\ targetRoster = "SameRoster"
  /\ successorActive
  =>
    /\ transitionAuthority = "RetainedSameRoster"
    /\ receiptStage = "Consumed"
    /\ receiptConsumeCount = 1
    /\ retainedSuccessor = ExpectedSuccessor

RetryChunkHasLiveOwnerInvariant ==
  retryableChunk = 1 =>
    /\ responderActive
    /\ responderWritable
    /\ responderAuthorized

DurableHighWaterFailClosedInvariant ==
  /\ (successorActive => serviceGeneration = durableHighWater)
  /\ (serviceGeneration > durableHighWater =>
        /\ restartRequired
        /\ ~successorActive
        /\ failureReason = "RollbackMismatch")
  /\ (restartRequired => ~successorActive)

HighWaterAheadSnapshotInvariant ==
  durableHighWater > serviceGeneration =>
    /\ ~successorActive
    /\ receiptStage = "Retained"
    /\ transitionAuthority = "None"
    /\ ~forcedTransitionUsed
    /\ (restartRequired =>
          failureReason \in
            {"SnapshotPersistenceFailure", "TornHighWaterSnapshot"})
    /\ (~restartRequired => failureReason = "None")

TornMidpointOpenRejectionInvariant ==
  tornMidpointOpenRejected =>
    /\ TornHighWaterSnapshotMidpoint
    /\ tornHighWaterHistory
    /\ ~successorActive

TornHighWaterHistoryOriginInvariant ==
  TornHighWaterSnapshotMidpoint => tornHighWaterHistory

ForeignOwnerCandidateRejectionInvariant ==
  /\ (foreignReceiptCandidatePresent =>
        /\ receiptStage = "Minted"
        /\ ExactSuccessorConstruction
        /\ foreignReceiptCandidateObserved
        /\ ~foreignReceiptRejected
        /\ ~restartRequired
        /\ ~successorActive
        /\ transitionAuthority = "None"
        /\ failureReason = "None")
  /\ (foreignReceiptRejected =>
        /\ ~foreignReceiptCandidatePresent
        /\ foreignReceiptCandidateObserved
        /\ receiptStage = "Minted"
        /\ ExactSuccessorConstruction
        /\ restartRequired
        /\ ~successorActive
        /\ transitionAuthority = "None"
        /\ failureReason = "ForeignOwnerMismatch")

PredecessorMismatchCandidateRejectionInvariant ==
  /\ (predecessorMismatchCandidateKind # "NoMismatch" =>
        /\ predecessorMismatchObservedKind =
             predecessorMismatchCandidateKind
        /\ ~predecessorMismatchRejected
        /\ receiptStage = "Minted"
        /\ ExactSuccessorConstruction
        /\ ~restartRequired
        /\ ~successorActive
        /\ transitionAuthority = "None"
        /\ failureReason = "None")
  /\ (predecessorMismatchRejected =>
        /\ predecessorMismatchCandidateKind = "NoMismatch"
        /\ predecessorMismatchObservedKind \in
             {"ContextMismatch", "ArtifactMismatch"}
        /\ receiptStage = "Minted"
        /\ ExactSuccessorConstruction
        /\ restartRequired
        /\ ~successorActive
        /\ transitionAuthority = "None"
        /\ failureReason =
             PredecessorMismatchFailure(predecessorMismatchObservedKind))

WrongSuccessorCandidateRejectionInvariant ==
  /\ (wrongSuccessorCandidatePresent =>
        /\ wrongSuccessorCandidateObserved
        /\ ~wrongSuccessorRejected
        /\ receiptStage = "Minted"
        /\ ExactSuccessorConstruction
        /\ ~restartRequired
        /\ ~successorActive
        /\ transitionAuthority = "None"
        /\ failureReason = "None")
  /\ (wrongSuccessorRejected =>
        /\ ~wrongSuccessorCandidatePresent
        /\ wrongSuccessorCandidateObserved
        /\ receiptStage = "Minted"
        /\ ExactSuccessorConstruction
        /\ restartRequired
        /\ ~successorActive
        /\ transitionAuthority = "None"
        /\ failureReason = "ImmediateSuccessorMismatch")

FailureLatchInvariant ==
  (failureReason = "None") <=> ~restartRequired

LateOldCallbackIsolationInvariant ==
  /\ successorCursor = 0
  /\ (lateOldCallbackObserved =>
        /\ targetRoster = "ChangedRoster"
        /\ transitionAuthority = "Typed"
        /\ receiptStage = "Consumed")

TypedRolloverSafetyInvariant ==
  /\ TypedRolloverTypeInvariant
  /\ ReceiptLifecycleInvariant
  /\ FinalSealRejectsLateEnqueueInvariant
  /\ ReceiptExactOwnerAndPredecessorInvariant
  /\ RetainedWrapperIdentityInvariant
  /\ TransitionAuthorityLifecycleInvariant
  /\ OrdinaryChangedRosterTransitionInvariant
  /\ LiveChangedRosterNeedsTypedReceiptInvariant
  /\ TypedTransitionAtomicClearInvariant
  /\ SameRosterTransportPreservationInvariant
  /\ SameRosterRetryPreservationInvariant
  /\ RetryChunkHasLiveOwnerInvariant
  /\ DurableHighWaterFailClosedInvariant
  /\ HighWaterAheadSnapshotInvariant
  /\ TornMidpointOpenRejectionInvariant
  /\ TornHighWaterHistoryOriginInvariant
  /\ ForeignOwnerCandidateRejectionInvariant
  /\ PredecessorMismatchCandidateRejectionInvariant
  /\ WrongSuccessorCandidateRejectionInvariant
  /\ FailureLatchInvariant
  /\ LateOldCallbackIsolationInvariant

TornHighWaterHistoryStepSafety ==
  /\ (tornHighWaterHistory => tornHighWaterHistory')
  /\ (tornHighWaterHistory /\ TornHighWaterSnapshotMidpoint =>
        /\ serviceGeneration' = serviceGeneration
        /\ durableHighWater' = durableHighWater
        /\ ~successorActive'
        /\ restartRequired'
        /\ failureReason' = failureReason)

ForeignOwnerCandidateStepSafety ==
  /\ (foreignReceiptCandidateObserved =>
        foreignReceiptCandidateObserved')
  /\ (foreignReceiptCandidatePresent =>
        /\ foreignReceiptCandidateObserved'
        /\ receiptStage' = "Minted"
        /\ ~successorActive'
        /\ transitionAuthority' = "None"
        /\ (~foreignReceiptCandidatePresent' =>
              /\ foreignReceiptRejected'
              /\ restartRequired'
              /\ failureReason' = "ForeignOwnerMismatch"))

PredecessorMismatchCandidateStepSafety ==
  /\ (predecessorMismatchObservedKind # "NoMismatch" =>
        predecessorMismatchObservedKind' =
          predecessorMismatchObservedKind)
  /\ (predecessorMismatchCandidateKind # "NoMismatch" =>
        /\ predecessorMismatchObservedKind' =
             predecessorMismatchObservedKind
        /\ receiptStage' = "Minted"
        /\ ~successorActive'
        /\ transitionAuthority' = "None"
        /\ predecessorMismatchCandidateKind' \in
             {predecessorMismatchCandidateKind, "NoMismatch"}
        /\ (predecessorMismatchCandidateKind' = "NoMismatch" =>
              /\ predecessorMismatchRejected'
              /\ restartRequired'
              /\ failureReason' =
                   PredecessorMismatchFailure(
                     predecessorMismatchCandidateKind)))

WrongSuccessorCandidateStepSafety ==
  /\ (wrongSuccessorCandidateObserved =>
        wrongSuccessorCandidateObserved')
  /\ (wrongSuccessorCandidatePresent =>
        /\ wrongSuccessorCandidateObserved'
        /\ receiptStage' = "Minted"
        /\ ~successorActive'
        /\ transitionAuthority' = "None"
        /\ (~wrongSuccessorCandidatePresent' =>
              /\ wrongSuccessorRejected'
              /\ restartRequired'
              /\ failureReason' = "ImmediateSuccessorMismatch"))

TornHighWaterHistoryActionProperty ==
  [][TornHighWaterHistoryStepSafety]_typedRolloverVars

ForeignOwnerCandidateActionProperty ==
  [][ForeignOwnerCandidateStepSafety]_typedRolloverVars

PredecessorMismatchCandidateActionProperty ==
  [][PredecessorMismatchCandidateStepSafety]_typedRolloverVars

WrongSuccessorCandidateActionProperty ==
  [][WrongSuccessorCandidateStepSafety]_typedRolloverVars

NoRolloverFailure ==
  [](/\ failureReason = "None"
     /\ ~restartRequired)

ChangedRosterSuccessorActiveWithoutRestart ==
  /\ targetRoster = "ChangedRoster"
  /\ successorActive
  /\ serviceGeneration = NextGeneration
  /\ ~restartRequired

ResponsiveChangedRosterRolloverLiveness ==
  NoRolloverFailure =>
    ((/\ targetRoster = "ChangedRoster"
      /\ finalityValidated)
      ~>
      ChangedRosterSuccessorActiveWithoutRestart)

=============================================================================
