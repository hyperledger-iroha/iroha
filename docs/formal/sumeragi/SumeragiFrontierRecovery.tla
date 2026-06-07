---- MODULE SumeragiFrontierRecovery ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for the Sumeragi/Taira frontier recovery hang class.

This model intentionally abstracts signatures and ECDSA into finite vote
evidence counters. It focuses on one active pending contiguous frontier block
plus one concrete future frontier slot, and the local state that can otherwise
pace or hide recovery:
- queued commit votes that have not reached the local quorum counter yet,
- local payload availability,
- stale vs. fresh frontier recovery ownership,
- quorum-reschedule marker/window state,
- future frontier evidence that can reanchor and promote the next slot,
- age- and event-scoped pending progress bookkeeping,
- stale frontier recovery ownership scoped by the subject view it rotated,
- deterministic commit, retransmit, view-rotation, and zero-evidence drop
  outcomes after GST.
***************************************************************************)

CONSTANTS
  \* @type: Int;
  N,
  \* @type: Int;
  CommitQuorum,
  \* @type: Int;
  MaxBacklog,
  \* @type: Int;
  MaxView,
  \* @type: Int;
  RescheduleWindow,
  \* @type: Int;
  MaxProgressAge,
  \* @type: Bool;
  BugDisableStaleRecovery,
  \* @type: Bool;
  BugKeepStaleRecoveryOwnerAfterUnlock,
  \* @type: Bool;
  BugDisableQueueDrain,
  \* @type: Bool;
  BugDisablePayloadRecovery,
  \* @type: Bool;
  BugKeepPayloadRecoveryOwner,
  \* @type: Bool;
  BugDisableRetransmitFollowthrough,
  \* @type: Bool;
  BugKeepQuorumWindowAfterRetransmit,
  \* @type: Bool;
  BugDropViewBoundRetransmitEvidence,
  \* @type: Bool;
  BugDisableFuturePromotion,
  \* @type: Bool;
  BugDisableFutureReanchorClear,
  \* @type: Bool;
  BugAllowFutureEvidenceDrop,
  \* @type: Bool;
  BugAllowZeroEvidenceFutureDrop,
  \* @type: Bool;
  BugMarkFutureReanchorRotated,
  \* @type: Bool;
  BugPreserveFutureReanchorActiveMarkers,
  \* @type: Bool;
  BugPromoteWithoutReset,
  \* @type: Bool;
  BugFutureStaleOwnerBlocksReanchor,
  \* @type: Bool;
  BugDisablePendingProgressTouch,
  \* @type: Bool;
  BugHeightOnlyStaleRecoveryUnlock

VARIABLES
  \* @type: Int;
  frontierSlot,
  \* @type: Bool;
  pending,
  \* @type: Bool;
  contiguous,
  \* @type: Bool;
  committed,
  \* @type: Bool;
  dropped,
  \* @type: Str;
  dropReason,
  \* @type: Int;
  commitVotes,
  \* @type: Int;
  queuedVotes,
  \* @type: Str;
  payloadState,
  \* @type: Str;
  recoveryOwner,
  \* @type: Bool;
  quorumRescheduleArmed,
  \* @type: Int;
  quorumWindowAge,
  \* @type: Int;
  view,
  \* @type: Int;
  subjectView,
  \* @type: Int;
  progressAge,
  \* @type: Str;
  lastProgressKind,
  \* @type: Str;
  validationState,
  \* @type: Bool;
  localVoteEmitted,
  \* @type: Bool;
  commitQcObserved,
  \* @type: Int;
  recoveryLastRotationView,
  \* @type: Bool;
  staleRecoveryUnlocked,
  \* @type: Bool;
  payloadRecovered,
  \* @type: Bool;
  quorumRetransmitted,
  \* @type: Bool;
  rotated,
  \* @type: Bool;
  futurePresent,
  \* @type: Bool;
  futureContiguous,
  \* @type: Int;
  futureCommitVotes,
  \* @type: Int;
  futureQueuedVotes,
  \* @type: Str;
  futurePayloadState,
  \* @type: Str;
  futureRecoveryOwner,
  \* @type: Bool;
  futurePromotionReady,
  \* @type: Bool;
  futurePromoted,
  \* @type: Bool;
  futureEvidenceObserved,
  \* @type: Bool;
  promotionFresh,
  \* @type: Bool;
  gst

vars == <<
  frontierSlot,
  pending,
  contiguous,
  committed,
  dropped,
  dropReason,
  commitVotes,
  queuedVotes,
  payloadState,
  recoveryOwner,
  quorumRescheduleArmed,
  quorumWindowAge,
  view,
  subjectView,
  progressAge,
  lastProgressKind,
  validationState,
  localVoteEmitted,
  commitQcObserved,
  recoveryLastRotationView,
  staleRecoveryUnlocked,
  payloadRecovered,
  quorumRetransmitted,
  rotated,
  futurePresent,
  futureContiguous,
  futureCommitVotes,
  futureQueuedVotes,
  futurePayloadState,
  futureRecoveryOwner,
  futurePromotionReady,
  futurePromoted,
  futureEvidenceObserved,
  promotionFresh,
  gst
>>

PayloadStates == {"Missing", "Local"}
RecoveryOwners == {"None", "Local", "Remote", "Stale"}
DropReasons == {"None", "ZeroEvidence", "ViewBound"}
ValidationStates == {"Pending", "Valid"}
ProgressEventKinds ==
  {"Validation", "LocalVote", "CommitQc", "VoteDrain", "PayloadRecovery",
   "StaleRecovery", "QuorumRetransmit", "FutureReanchor", "Promotion",
   "ViewRotation"}
ProgressKinds == ProgressEventKinds \cup {"None"}

ActiveFrontier ==
  /\ pending
  /\ contiguous
  /\ ~committed
  /\ ~dropped

VoteBacked == commitVotes + queuedVotes > 0
FullQuorum == commitVotes >= CommitQuorum
PayloadAvailable == payloadState = "Local"
FreshRecoveryOwner == recoveryOwner # "Stale"
ValidationReady == validationState = "Valid"
StaleRecoveryViewCovered == recoveryLastRotationView >= subjectView
NextProgressAge == IF progressAge < MaxProgressAge THEN progressAge + 1 ELSE MaxProgressAge

RecordProgress(kind) ==
  /\ progressAge' = IF BugDisablePendingProgressTouch THEN progressAge ELSE 0
  /\ lastProgressKind' = kind

ClearProgressEvent ==
  /\ progressAge' = progressAge
  /\ lastProgressKind' = "None"

AgeWithoutProgress ==
  /\ progressAge' = NextProgressAge
  /\ lastProgressKind' = "None"

PendingProgressFlags == <<validationState, localVoteEmitted, commitQcObserved>>
ViewRecoveryBookkeeping == <<subjectView, recoveryLastRotationView, staleRecoveryUnlocked>>

FutureVoteBacked == futureCommitVotes + futureQueuedVotes > 0
FutureFrontierEvidence ==
  /\ futurePresent
  /\ futureContiguous
  /\ FutureVoteBacked

FutureReanchorReadyForCurrent ==
  /\ ActiveFrontier
  /\ FreshRecoveryOwner
  /\ FutureFrontierEvidence

PromotedSecondSlotPending ==
  /\ frontierSlot = 1
  /\ ActiveFrontier
  /\ VoteBacked

TypeInvariant ==
  /\ N \in Nat
  /\ CommitQuorum \in 1..N
  /\ MaxBacklog \in 0..N
  /\ MaxView \in Nat
  /\ RescheduleWindow \in Nat
  /\ MaxProgressAge \in Nat
  /\ MaxProgressAge > 0
  /\ BugDisableStaleRecovery \in BOOLEAN
  /\ BugKeepStaleRecoveryOwnerAfterUnlock \in BOOLEAN
  /\ BugDisableQueueDrain \in BOOLEAN
  /\ BugDisablePayloadRecovery \in BOOLEAN
  /\ BugKeepPayloadRecoveryOwner \in BOOLEAN
  /\ BugDisableRetransmitFollowthrough \in BOOLEAN
  /\ BugKeepQuorumWindowAfterRetransmit \in BOOLEAN
  /\ BugDropViewBoundRetransmitEvidence \in BOOLEAN
  /\ BugDisableFuturePromotion \in BOOLEAN
  /\ BugDisableFutureReanchorClear \in BOOLEAN
  /\ BugAllowFutureEvidenceDrop \in BOOLEAN
  /\ BugAllowZeroEvidenceFutureDrop \in BOOLEAN
  /\ BugMarkFutureReanchorRotated \in BOOLEAN
  /\ BugPreserveFutureReanchorActiveMarkers \in BOOLEAN
  /\ BugPromoteWithoutReset \in BOOLEAN
  /\ BugFutureStaleOwnerBlocksReanchor \in BOOLEAN
  /\ BugDisablePendingProgressTouch \in BOOLEAN
  /\ BugHeightOnlyStaleRecoveryUnlock \in BOOLEAN
  /\ frontierSlot \in 0..1
  /\ pending \in BOOLEAN
  /\ contiguous \in BOOLEAN
  /\ committed \in BOOLEAN
  /\ dropped \in BOOLEAN
  /\ dropReason \in DropReasons
  /\ commitVotes \in 0..N
  /\ queuedVotes \in 0..MaxBacklog
  /\ commitVotes + queuedVotes <= N
  /\ payloadState \in PayloadStates
  /\ recoveryOwner \in RecoveryOwners
  /\ quorumRescheduleArmed \in BOOLEAN
  /\ quorumWindowAge \in 0..RescheduleWindow
  /\ quorumRescheduleArmed \/ quorumWindowAge = 0
  /\ view \in 0..MaxView
  /\ subjectView \in 0..MaxView
  /\ pending => subjectView = view
  /\ progressAge \in 0..MaxProgressAge
  /\ lastProgressKind \in ProgressKinds
  /\ validationState \in ValidationStates
  /\ localVoteEmitted \in BOOLEAN
  /\ commitQcObserved \in BOOLEAN
  /\ commitQcObserved => FullQuorum
  /\ recoveryLastRotationView \in 0..MaxView
  /\ recoveryOwner = "Stale" /\ ~BugHeightOnlyStaleRecoveryUnlock => StaleRecoveryViewCovered
  /\ staleRecoveryUnlocked \in BOOLEAN
  /\ payloadRecovered \in BOOLEAN
  /\ quorumRetransmitted \in BOOLEAN
  /\ rotated \in BOOLEAN
  /\ futurePresent \in BOOLEAN
  /\ futureContiguous \in BOOLEAN
  /\ futureCommitVotes \in 0..N
  /\ futureQueuedVotes \in 0..MaxBacklog
  /\ futureCommitVotes + futureQueuedVotes <= N
  /\ futurePayloadState \in PayloadStates
  /\ futureRecoveryOwner \in RecoveryOwners
  /\ futurePromotionReady \in BOOLEAN
  /\ futurePromoted \in BOOLEAN
  /\ futureEvidenceObserved \in BOOLEAN
  /\ promotionFresh \in BOOLEAN
  /\ futurePresent =>
       /\ futureContiguous
       /\ FutureVoteBacked
       /\ futureEvidenceObserved
  /\ ~futurePresent =>
       /\ ~futureContiguous
       /\ futureCommitVotes = 0
       /\ futureQueuedVotes = 0
       /\ futurePayloadState = "Missing"
       /\ futureRecoveryOwner = "None"
  /\ futurePromotionReady =>
       /\ frontierSlot = 0
       /\ ~pending
       /\ FutureFrontierEvidence
       /\ ~futurePromoted
       /\ futureEvidenceObserved
  /\ futurePromoted =>
       /\ frontierSlot = 1
       /\ ~futurePresent
       /\ ~futurePromotionReady
       /\ futureEvidenceObserved
  /\ futureEvidenceObserved /\ ~futurePromoted => FutureFrontierEvidence
  /\ promotionFresh =>
       /\ frontierSlot = 1
       /\ pending
       /\ futurePromoted
  /\ frontierSlot = 1 => ~futurePresent
  /\ committed => ~pending
  /\ rotated => ~pending
  /\ gst \in BOOLEAN

Init ==
  /\ frontierSlot = 0
  /\ pending = TRUE
  /\ contiguous = TRUE
  /\ committed = FALSE
  /\ dropped = FALSE
  /\ dropReason = "None"
  /\ commitVotes \in 0..N
  /\ queuedVotes \in 0..MaxBacklog
  /\ commitVotes + queuedVotes <= N
  /\ payloadState \in PayloadStates
  /\ recoveryOwner \in RecoveryOwners
  /\ quorumRescheduleArmed \in BOOLEAN
  /\ IF quorumRescheduleArmed
     THEN quorumWindowAge \in 0..RescheduleWindow
     ELSE quorumWindowAge = 0
  /\ view \in 0..MaxView
  /\ subjectView = view
  /\ progressAge \in 0..MaxProgressAge
  /\ lastProgressKind = "None"
  /\ validationState \in ValidationStates
  /\ localVoteEmitted \in BOOLEAN
  /\ commitQcObserved \in BOOLEAN
  /\ commitQcObserved => commitVotes >= CommitQuorum
  /\ recoveryLastRotationView \in 0..MaxView
  /\ IF recoveryOwner = "Stale" /\ ~BugHeightOnlyStaleRecoveryUnlock
     THEN recoveryLastRotationView \in subjectView..MaxView
     ELSE TRUE
  /\ staleRecoveryUnlocked = FALSE
  /\ payloadRecovered = FALSE
  /\ quorumRetransmitted = FALSE
  /\ rotated = FALSE
  /\ futurePresent \in BOOLEAN
  /\ IF futurePresent
     THEN /\ futureContiguous = TRUE
          /\ futureCommitVotes \in 0..N
          /\ futureQueuedVotes \in 0..MaxBacklog
          /\ futureCommitVotes + futureQueuedVotes <= N
          /\ futureCommitVotes + futureQueuedVotes > 0
          /\ futurePayloadState \in PayloadStates
          /\ futureRecoveryOwner \in RecoveryOwners
     ELSE /\ futureContiguous = FALSE
          /\ futureCommitVotes = 0
          /\ futureQueuedVotes = 0
          /\ futurePayloadState = "Missing"
          /\ futureRecoveryOwner = "None"
  /\ futurePromotionReady = FALSE
  /\ futurePromoted = FALSE
  /\ futureEvidenceObserved = futurePresent
  /\ promotionFresh = FALSE
  /\ gst = FALSE

TlcNoFutureInit ==
  futurePresent = FALSE

TlcFutureInit(fCommitVotes, fQueuedVotes, fPayloadState, fRecoveryOwner) ==
  /\ futurePresent = TRUE
  /\ futureCommitVotes = fCommitVotes
  /\ futureQueuedVotes = fQueuedVotes
  /\ futurePayloadState = fPayloadState
  /\ futureRecoveryOwner = fRecoveryOwner

TlcFastCommonInit ==
  /\ progressAge = 0
  /\ recoveryLastRotationView = view

TlcFastCanonicalInit ==
  /\ TlcFastCommonInit
  /\ \/
        \* Terminal zero-evidence branch after GST.
        /\ commitVotes = 0
        /\ queuedVotes = 0
        /\ payloadState = "Missing"
        /\ recoveryOwner = "None"
        /\ quorumRescheduleArmed = FALSE
        /\ quorumWindowAge = 0
        /\ view = 0
        /\ validationState = "Pending"
        /\ localVoteEmitted = FALSE
        /\ commitQcObserved = FALSE
        /\ TlcNoFutureInit
     \/
        \* Direct quorum/payload/validation commit branch.
        /\ commitVotes = CommitQuorum
        /\ queuedVotes = 0
        /\ payloadState = "Local"
        /\ recoveryOwner = "None"
        /\ quorumRescheduleArmed = FALSE
        /\ quorumWindowAge = 0
        /\ view = 0
        /\ validationState = "Valid"
        /\ localVoteEmitted = TRUE
        /\ commitQcObserved = TRUE
        /\ TlcNoFutureInit
     \/
        \* Queued-vote, validation, payload-recovery, and commit branch.
        /\ commitVotes = CommitQuorum - 1
        /\ queuedVotes = 1
        /\ payloadState = "Missing"
        /\ recoveryOwner = "None"
        /\ quorumRescheduleArmed = FALSE
        /\ quorumWindowAge = 0
        /\ view = 0
        /\ validationState = "Pending"
        /\ localVoteEmitted = FALSE
        /\ commitQcObserved = FALSE
        /\ TlcNoFutureInit
     \/
        \* Stale recovery-owner unlock branch.
        /\ commitVotes = 1
        /\ queuedVotes = 0
        /\ payloadState = "Local"
        /\ recoveryOwner = "Stale"
        /\ quorumRescheduleArmed = FALSE
        /\ quorumWindowAge = 0
        /\ view = 1
        /\ validationState = "Valid"
        /\ localVoteEmitted = TRUE
        /\ commitQcObserved = FALSE
        /\ TlcNoFutureInit
     \/
        \* Retransmit then lower-view rotation branch.
        /\ commitVotes = 1
        /\ queuedVotes = 0
        /\ payloadState = "Local"
        /\ recoveryOwner = "None"
        /\ quorumRescheduleArmed = TRUE
        /\ quorumWindowAge = RescheduleWindow
        /\ view = 0
        /\ validationState = "Valid"
        /\ localVoteEmitted = TRUE
        /\ commitQcObserved = FALSE
        /\ TlcNoFutureInit
     \/
        \* Retransmit then view-bound drop branch.
        /\ commitVotes = 1
        /\ queuedVotes = 0
        /\ payloadState = "Local"
        /\ recoveryOwner = "None"
        /\ quorumRescheduleArmed = TRUE
        /\ quorumWindowAge = RescheduleWindow
        /\ view = MaxView
        /\ validationState = "Valid"
        /\ localVoteEmitted = TRUE
        /\ commitQcObserved = FALSE
        /\ TlcNoFutureInit
     \/
        \* Future frontier reanchor and promotion branch.
        /\ commitVotes = 1
        /\ queuedVotes = 0
        /\ payloadState = "Local"
        /\ recoveryOwner = "None"
        /\ quorumRescheduleArmed = FALSE
        /\ quorumWindowAge = 0
        /\ view = 0
        /\ validationState = "Valid"
        /\ localVoteEmitted = TRUE
        /\ commitQcObserved = FALSE
        /\ TlcFutureInit(1, 0, "Missing", "None")

ZeroEvidenceFutureDropBugInit ==
  /\ Init
  /\ TlcFastCommonInit
  /\ commitVotes = 0
  /\ queuedVotes = 0
  /\ payloadState = "Missing"
  /\ recoveryOwner = "Stale"
  /\ quorumRescheduleArmed = FALSE
  /\ quorumWindowAge = 0
  /\ view = 0
  /\ validationState = "Pending"
  /\ localVoteEmitted = FALSE
  /\ commitQcObserved = FALSE
  /\ TlcFutureInit(0, 1, "Missing", "Local")

FutureReanchorRotationBugInit ==
  /\ Init
  /\ TlcFastCommonInit
  /\ commitVotes = 1
  /\ queuedVotes = 0
  /\ payloadState = "Local"
  /\ recoveryOwner = "None"
  /\ quorumRescheduleArmed = FALSE
  /\ quorumWindowAge = 0
  /\ view = 0
  /\ validationState = "Valid"
  /\ localVoteEmitted = TRUE
  /\ commitQcObserved = FALSE
  /\ TlcFutureInit(1, 0, "Missing", "None")

FutureReanchorActiveMarkerBugInit ==
  /\ Init
  /\ TlcFastCommonInit
  /\ commitVotes = 1
  /\ queuedVotes = 0
  /\ payloadState = "Missing"
  /\ recoveryOwner = "None"
  /\ quorumRescheduleArmed = FALSE
  /\ quorumWindowAge = 0
  /\ view = 0
  /\ validationState = "Pending"
  /\ localVoteEmitted = FALSE
  /\ commitQcObserved = FALSE
  /\ TlcNoFutureInit

QuorumRetransmitWindowCleanupBugInit ==
  /\ Init
  /\ TlcFastCommonInit
  /\ commitVotes = 1
  /\ queuedVotes = 0
  /\ payloadState = "Local"
  /\ recoveryOwner = "None"
  /\ quorumRescheduleArmed = TRUE
  /\ quorumWindowAge = RescheduleWindow
  /\ view = 0
  /\ validationState = "Valid"
  /\ localVoteEmitted = TRUE
  /\ commitQcObserved = FALSE
  /\ TlcNoFutureInit

PayloadRecoveryOwnerBugInit ==
  /\ Init
  /\ TlcFastCommonInit
  /\ commitVotes = 1
  /\ queuedVotes = 0
  /\ payloadState = "Missing"
  /\ recoveryOwner = "Remote"
  /\ quorumRescheduleArmed = FALSE
  /\ quorumWindowAge = 0
  /\ view = 0
  /\ validationState = "Pending"
  /\ localVoteEmitted = FALSE
  /\ commitQcObserved = FALSE
  /\ TlcNoFutureInit

StaleRecoveryUnlockOwnerBugInit ==
  /\ Init
  /\ TlcFastCommonInit
  /\ commitVotes = 1
  /\ queuedVotes = 0
  /\ payloadState = "Local"
  /\ recoveryOwner = "Stale"
  /\ quorumRescheduleArmed = FALSE
  /\ quorumWindowAge = 0
  /\ view = 1
  /\ validationState = "Valid"
  /\ localVoteEmitted = TRUE
  /\ commitQcObserved = FALSE
  /\ TlcNoFutureInit

ViewBoundRetransmitEvidenceBugInit ==
  /\ Init
  /\ TlcFastCommonInit
  /\ commitVotes = 1
  /\ queuedVotes = 0
  /\ payloadState = "Local"
  /\ recoveryOwner = "None"
  /\ quorumRescheduleArmed = TRUE
  /\ quorumWindowAge = RescheduleWindow
  /\ view = MaxView
  /\ validationState = "Valid"
  /\ localVoteEmitted = TRUE
  /\ commitQcObserved = FALSE
  /\ TlcNoFutureInit

ClearStaleRecoveryEnabled ==
  /\ ~BugDisableStaleRecovery
  /\ ~(BugFutureStaleOwnerBlocksReanchor /\ FutureFrontierEvidence)
  /\ gst
  /\ ActiveFrontier
  /\ recoveryOwner = "Stale"
  /\ StaleRecoveryViewCovered \/ BugHeightOnlyStaleRecoveryUnlock

ValidatePendingEnabled ==
  /\ gst
  /\ ActiveFrontier
  /\ FreshRecoveryOwner
  /\ ~FutureReanchorReadyForCurrent
  /\ validationState = "Pending"

EmitLocalCommitVoteEnabled ==
  /\ gst
  /\ ActiveFrontier
  /\ FreshRecoveryOwner
  /\ ~FutureReanchorReadyForCurrent
  /\ ValidationReady
  /\ ~localVoteEmitted

ObserveCommitQcEnabled ==
  /\ gst
  /\ ActiveFrontier
  /\ FreshRecoveryOwner
  /\ ~FutureReanchorReadyForCurrent
  /\ FullQuorum
  /\ ~commitQcObserved

DrainVoteQueueEnabled ==
  /\ ~BugDisableQueueDrain
  /\ gst
  /\ ActiveFrontier
  /\ FreshRecoveryOwner
  /\ ~FutureReanchorReadyForCurrent
  /\ queuedVotes > 0

RecoverPayloadEnabled ==
  /\ ~BugDisablePayloadRecovery
  /\ gst
  /\ ActiveFrontier
  /\ FreshRecoveryOwner
  /\ ~FutureReanchorReadyForCurrent
  /\ queuedVotes = 0
  /\ payloadState = "Missing"

CommitFrontierEnabled ==
  /\ gst
  /\ ActiveFrontier
  /\ FreshRecoveryOwner
  /\ ~FutureReanchorReadyForCurrent
  /\ ValidationReady
  /\ PayloadAvailable
  /\ FullQuorum

ArmQuorumRescheduleEnabled ==
  /\ gst
  /\ ActiveFrontier
  /\ VoteBacked
  /\ FreshRecoveryOwner
  /\ ~FutureReanchorReadyForCurrent
  /\ queuedVotes = 0
  /\ PayloadAvailable
  /\ ~FullQuorum
  /\ ~quorumRescheduleArmed
  /\ ~quorumRetransmitted

QuorumWindowTickEnabled ==
  /\ gst
  /\ ActiveFrontier
  /\ VoteBacked
  /\ FreshRecoveryOwner
  /\ ~FutureReanchorReadyForCurrent
  /\ PayloadAvailable
  /\ ~FullQuorum
  /\ quorumRescheduleArmed
  /\ quorumWindowAge < RescheduleWindow

QuorumRetransmitEnabled ==
  /\ gst
  /\ ActiveFrontier
  /\ VoteBacked
  /\ FreshRecoveryOwner
  /\ ~FutureReanchorReadyForCurrent
  /\ PayloadAvailable
  /\ ~FullQuorum
  /\ quorumRescheduleArmed
  /\ quorumWindowAge >= RescheduleWindow

RotateVoteBackedFrontierEnabled ==
  /\ ~BugDisableRetransmitFollowthrough
  /\ gst
  /\ ActiveFrontier
  /\ VoteBacked
  /\ FreshRecoveryOwner
  /\ ~FutureReanchorReadyForCurrent
  /\ PayloadAvailable
  /\ ~FullQuorum
  /\ ~quorumRescheduleArmed
  /\ quorumRetransmitted
  /\ view < MaxView

DropAtViewBoundEnabled ==
  /\ ~BugDisableRetransmitFollowthrough
  /\ gst
  /\ ActiveFrontier
  /\ VoteBacked
  /\ FreshRecoveryOwner
  /\ ~FutureReanchorReadyForCurrent
  /\ PayloadAvailable
  /\ ~FullQuorum
  /\ ~quorumRescheduleArmed
  /\ quorumRetransmitted
  /\ view = MaxView

ClearCurrentForFutureReanchorEnabled ==
  /\ ~BugDisableFutureReanchorClear
  /\ gst
  /\ ActiveFrontier
  /\ FreshRecoveryOwner
  /\ FutureFrontierEvidence
  /\ ~futurePromotionReady

PromoteFutureFrontierEnabled ==
  /\ ~BugDisableFuturePromotion
  /\ gst
  /\ frontierSlot = 0
  /\ futurePromotionReady
  /\ FutureFrontierEvidence

DropZeroEvidenceEnabled ==
  /\ gst
  /\ ActiveFrontier
  /\ ~FutureReanchorReadyForCurrent
  /\ (~FutureFrontierEvidence \/ BugAllowZeroEvidenceFutureDrop)
  /\ ~VoteBacked

LearnFutureFrontierEvidenceEnabled ==
  /\ gst
  /\ frontierSlot = 0
  /\ ActiveFrontier
  /\ ~futurePresent
  /\ ~futurePromotionReady
  /\ ~futurePromoted

DropFutureFrontierEvidenceEnabled ==
  /\ BugAllowFutureEvidenceDrop
  /\ gst
  /\ FutureFrontierEvidence
  /\ ~futurePromoted

FrontierProgressEnabled ==
  \/ ClearStaleRecoveryEnabled
  \/ ValidatePendingEnabled
  \/ EmitLocalCommitVoteEnabled
  \/ ObserveCommitQcEnabled
  \/ DrainVoteQueueEnabled
  \/ RecoverPayloadEnabled
  \/ CommitFrontierEnabled
  \/ ArmQuorumRescheduleEnabled
  \/ QuorumWindowTickEnabled
  \/ QuorumRetransmitEnabled
  \/ RotateVoteBackedFrontierEnabled
  \/ DropAtViewBoundEnabled
  \/ ClearCurrentForFutureReanchorEnabled
  \/ LearnFutureFrontierEvidenceEnabled

StallBeforeGst ==
  /\ ~gst
  /\ UNCHANGED vars

GstElapsed ==
  /\ ~gst
  /\ gst' = TRUE
  /\ UNCHANGED <<
      frontierSlot,
      pending,
      contiguous,
      committed,
      dropped,
      dropReason,
      commitVotes,
      queuedVotes,
      payloadState,
      recoveryOwner,
      quorumRescheduleArmed,
      quorumWindowAge,
      view,
      subjectView,
      progressAge,
      lastProgressKind,
      validationState,
      localVoteEmitted,
      commitQcObserved,
      recoveryLastRotationView,
      staleRecoveryUnlocked,
      payloadRecovered,
      quorumRetransmitted,
      rotated,
      futurePresent,
      futureContiguous,
      futureCommitVotes,
      futureQueuedVotes,
      futurePayloadState,
      futureRecoveryOwner,
      futurePromotionReady,
      futurePromoted,
      futureEvidenceObserved,
      promotionFresh
     >>

LearnFutureFrontierEvidence ==
  /\ LearnFutureFrontierEvidenceEnabled
  /\ ClearProgressEvent
  /\ futurePresent' = TRUE
  /\ futureContiguous' = TRUE
  /\ futureCommitVotes' = 1
  /\ futureQueuedVotes' = 0
  /\ futurePayloadState' = "Missing"
  /\ futureRecoveryOwner' = "None"
  /\ futureEvidenceObserved' = TRUE
  /\ UNCHANGED PendingProgressFlags
  /\ UNCHANGED ViewRecoveryBookkeeping
  /\ UNCHANGED <<
      frontierSlot,
      pending,
      contiguous,
      committed,
      dropped,
      dropReason,
      commitVotes,
      queuedVotes,
      payloadState,
      recoveryOwner,
      quorumRescheduleArmed,
      quorumWindowAge,
      view,
      payloadRecovered,
      quorumRetransmitted,
      rotated,
      futurePromotionReady,
      futurePromoted,
      promotionFresh,
      gst
     >>

ValidatePending ==
  /\ ValidatePendingEnabled
  /\ RecordProgress("Validation")
  /\ validationState' = "Valid"
  /\ promotionFresh' = FALSE
  /\ UNCHANGED <<
      frontierSlot,
      pending,
      contiguous,
      committed,
      dropped,
      dropReason,
      commitVotes,
      queuedVotes,
      payloadState,
      recoveryOwner,
      quorumRescheduleArmed,
      quorumWindowAge,
      view,
      localVoteEmitted,
      commitQcObserved,
      subjectView,
      recoveryLastRotationView,
      staleRecoveryUnlocked,
      payloadRecovered,
      quorumRetransmitted,
      rotated,
      futurePresent,
      futureContiguous,
      futureCommitVotes,
      futureQueuedVotes,
      futurePayloadState,
      futureRecoveryOwner,
      futurePromotionReady,
      futurePromoted,
      futureEvidenceObserved,
      gst
     >>

EmitLocalCommitVote ==
  /\ EmitLocalCommitVoteEnabled
  /\ RecordProgress("LocalVote")
  /\ localVoteEmitted' = TRUE
  /\ promotionFresh' = FALSE
  /\ UNCHANGED <<
      frontierSlot,
      pending,
      contiguous,
      committed,
      dropped,
      dropReason,
      commitVotes,
      queuedVotes,
      payloadState,
      recoveryOwner,
      quorumRescheduleArmed,
      quorumWindowAge,
      view,
      subjectView,
      validationState,
      commitQcObserved,
      recoveryLastRotationView,
      staleRecoveryUnlocked,
      payloadRecovered,
      quorumRetransmitted,
      rotated,
      futurePresent,
      futureContiguous,
      futureCommitVotes,
      futureQueuedVotes,
      futurePayloadState,
      futureRecoveryOwner,
      futurePromotionReady,
      futurePromoted,
      futureEvidenceObserved,
      gst
     >>

ObserveCommitQc ==
  /\ ObserveCommitQcEnabled
  /\ RecordProgress("CommitQc")
  /\ commitQcObserved' = TRUE
  /\ promotionFresh' = FALSE
  /\ UNCHANGED <<
      frontierSlot,
      pending,
      contiguous,
      committed,
      dropped,
      dropReason,
      commitVotes,
      queuedVotes,
      payloadState,
      recoveryOwner,
      quorumRescheduleArmed,
      quorumWindowAge,
      view,
      subjectView,
      validationState,
      localVoteEmitted,
      recoveryLastRotationView,
      staleRecoveryUnlocked,
      payloadRecovered,
      quorumRetransmitted,
      rotated,
      futurePresent,
      futureContiguous,
      futureCommitVotes,
      futureQueuedVotes,
      futurePayloadState,
      futureRecoveryOwner,
      futurePromotionReady,
      futurePromoted,
      futureEvidenceObserved,
      gst
     >>

ClearStaleRecovery ==
  /\ ClearStaleRecoveryEnabled
  /\ RecordProgress("StaleRecovery")
  /\ recoveryOwner' =
       IF BugKeepStaleRecoveryOwnerAfterUnlock THEN recoveryOwner ELSE "None"
  /\ staleRecoveryUnlocked' = TRUE
  /\ promotionFresh' = FALSE
  /\ UNCHANGED PendingProgressFlags
  /\ UNCHANGED <<
      frontierSlot,
      pending,
      contiguous,
      committed,
      dropped,
      dropReason,
      commitVotes,
      queuedVotes,
      payloadState,
      quorumRescheduleArmed,
      quorumWindowAge,
      view,
      subjectView,
      recoveryLastRotationView,
      payloadRecovered,
      quorumRetransmitted,
      rotated,
      futurePresent,
      futureContiguous,
      futureCommitVotes,
      futureQueuedVotes,
      futurePayloadState,
      futureRecoveryOwner,
      futurePromotionReady,
      futurePromoted,
      futureEvidenceObserved,
      gst
     >>

DrainVoteQueue ==
  /\ DrainVoteQueueEnabled
  /\ RecordProgress("VoteDrain")
  /\ commitVotes' = commitVotes + 1
  /\ queuedVotes' = queuedVotes - 1
  /\ promotionFresh' = FALSE
  /\ UNCHANGED PendingProgressFlags
  /\ UNCHANGED ViewRecoveryBookkeeping
  /\ UNCHANGED <<
      frontierSlot,
      pending,
      contiguous,
      committed,
      dropped,
      dropReason,
      payloadState,
      recoveryOwner,
      quorumRescheduleArmed,
      quorumWindowAge,
      view,
      payloadRecovered,
      quorumRetransmitted,
      rotated,
      futurePresent,
      futureContiguous,
      futureCommitVotes,
      futureQueuedVotes,
      futurePayloadState,
      futureRecoveryOwner,
      futurePromotionReady,
      futurePromoted,
      futureEvidenceObserved,
      gst
     >>

RecoverPayload ==
  /\ RecoverPayloadEnabled
  /\ RecordProgress("PayloadRecovery")
  /\ payloadState' = "Local"
  /\ payloadRecovered' = TRUE
  /\ recoveryOwner' = IF BugKeepPayloadRecoveryOwner THEN recoveryOwner ELSE "Local"
  /\ promotionFresh' = FALSE
  /\ UNCHANGED PendingProgressFlags
  /\ UNCHANGED ViewRecoveryBookkeeping
  /\ UNCHANGED <<
      frontierSlot,
      pending,
      contiguous,
      committed,
      dropped,
      dropReason,
      commitVotes,
      queuedVotes,
      quorumRescheduleArmed,
      quorumWindowAge,
      view,
      quorumRetransmitted,
      rotated,
      futurePresent,
      futureContiguous,
      futureCommitVotes,
      futureQueuedVotes,
      futurePayloadState,
      futureRecoveryOwner,
      futurePromotionReady,
      futurePromoted,
      futureEvidenceObserved,
      gst
     >>

CommitFrontier ==
  /\ CommitFrontierEnabled
  /\ ClearProgressEvent
  /\ committed' = TRUE
  /\ pending' = FALSE
  /\ quorumRescheduleArmed' = FALSE
  /\ quorumWindowAge' = 0
  /\ promotionFresh' = FALSE
  /\ UNCHANGED PendingProgressFlags
  /\ UNCHANGED ViewRecoveryBookkeeping
  /\ UNCHANGED <<
      frontierSlot,
      contiguous,
      dropped,
      dropReason,
      commitVotes,
      queuedVotes,
      payloadState,
      recoveryOwner,
      view,
      payloadRecovered,
      quorumRetransmitted,
      rotated,
      futurePresent,
      futureContiguous,
      futureCommitVotes,
      futureQueuedVotes,
      futurePayloadState,
      futureRecoveryOwner,
      futurePromotionReady,
      futurePromoted,
      futureEvidenceObserved,
      gst
     >>

ArmQuorumReschedule ==
  /\ ArmQuorumRescheduleEnabled
  /\ ClearProgressEvent
  /\ quorumRescheduleArmed' = TRUE
  /\ quorumWindowAge' = 0
  /\ promotionFresh' = FALSE
  /\ UNCHANGED PendingProgressFlags
  /\ UNCHANGED ViewRecoveryBookkeeping
  /\ UNCHANGED <<
      frontierSlot,
      pending,
      contiguous,
      committed,
      dropped,
      dropReason,
      commitVotes,
      queuedVotes,
      payloadState,
      recoveryOwner,
      view,
      payloadRecovered,
      quorumRetransmitted,
      rotated,
      futurePresent,
      futureContiguous,
      futureCommitVotes,
      futureQueuedVotes,
      futurePayloadState,
      futureRecoveryOwner,
      futurePromotionReady,
      futurePromoted,
      futureEvidenceObserved,
      gst
     >>

QuorumWindowTick ==
  /\ QuorumWindowTickEnabled
  /\ AgeWithoutProgress
  /\ quorumWindowAge' = quorumWindowAge + 1
  /\ promotionFresh' = FALSE
  /\ UNCHANGED PendingProgressFlags
  /\ UNCHANGED ViewRecoveryBookkeeping
  /\ UNCHANGED <<
      frontierSlot,
      pending,
      contiguous,
      committed,
      dropped,
      dropReason,
      commitVotes,
      queuedVotes,
      payloadState,
      recoveryOwner,
      quorumRescheduleArmed,
      view,
      payloadRecovered,
      quorumRetransmitted,
      rotated,
      futurePresent,
      futureContiguous,
      futureCommitVotes,
      futureQueuedVotes,
      futurePayloadState,
      futureRecoveryOwner,
      futurePromotionReady,
      futurePromoted,
      futureEvidenceObserved,
      gst
     >>

QuorumRetransmit ==
  /\ QuorumRetransmitEnabled
  /\ RecordProgress("QuorumRetransmit")
  /\ quorumRetransmitted' = TRUE
  /\ quorumRescheduleArmed' = BugKeepQuorumWindowAfterRetransmit
  /\ quorumWindowAge' =
       IF BugKeepQuorumWindowAfterRetransmit THEN quorumWindowAge ELSE 0
  /\ promotionFresh' = FALSE
  /\ UNCHANGED PendingProgressFlags
  /\ UNCHANGED ViewRecoveryBookkeeping
  /\ UNCHANGED <<
      frontierSlot,
      pending,
      contiguous,
      committed,
      dropped,
      dropReason,
      commitVotes,
      queuedVotes,
      payloadState,
      recoveryOwner,
      view,
      payloadRecovered,
      rotated,
      futurePresent,
      futureContiguous,
      futureCommitVotes,
      futureQueuedVotes,
      futurePayloadState,
      futureRecoveryOwner,
      futurePromotionReady,
      futurePromoted,
      futureEvidenceObserved,
      gst
     >>

RotateVoteBackedFrontier ==
  /\ RotateVoteBackedFrontierEnabled
  /\ RecordProgress("ViewRotation")
  /\ view' = view + 1
  /\ subjectView' = view + 1
  /\ recoveryLastRotationView' = view + 1
  /\ staleRecoveryUnlocked' = FALSE
  /\ rotated' = TRUE
  /\ pending' = FALSE
  /\ quorumRescheduleArmed' = FALSE
  /\ quorumWindowAge' = 0
  /\ promotionFresh' = FALSE
  /\ UNCHANGED PendingProgressFlags
  /\ UNCHANGED <<
      frontierSlot,
      contiguous,
      committed,
      dropped,
      dropReason,
      commitVotes,
      queuedVotes,
      payloadState,
      recoveryOwner,
      payloadRecovered,
      quorumRetransmitted,
      futurePresent,
      futureContiguous,
      futureCommitVotes,
      futureQueuedVotes,
      futurePayloadState,
      futureRecoveryOwner,
      futurePromotionReady,
      futurePromoted,
      futureEvidenceObserved,
      gst
     >>

DropAtViewBound ==
  /\ DropAtViewBoundEnabled
  /\ ClearProgressEvent
  /\ pending' = FALSE
  /\ dropped' = TRUE
  /\ dropReason' = "ViewBound"
  /\ rotated' = TRUE
  /\ quorumRescheduleArmed' = FALSE
  /\ quorumWindowAge' = 0
  /\ quorumRetransmitted' =
       IF BugDropViewBoundRetransmitEvidence THEN FALSE ELSE quorumRetransmitted
  /\ promotionFresh' = FALSE
  /\ UNCHANGED PendingProgressFlags
  /\ UNCHANGED ViewRecoveryBookkeeping
  /\ UNCHANGED <<
      frontierSlot,
      contiguous,
      committed,
      commitVotes,
      queuedVotes,
      payloadState,
      recoveryOwner,
      view,
      payloadRecovered,
      futurePresent,
      futureContiguous,
      futureCommitVotes,
      futureQueuedVotes,
      futurePayloadState,
      futureRecoveryOwner,
      futurePromotionReady,
      futurePromoted,
      futureEvidenceObserved,
      gst
     >>

ClearCurrentForFutureReanchor ==
  /\ ClearCurrentForFutureReanchorEnabled
  /\ RecordProgress("FutureReanchor")
  /\ pending' = FALSE
  /\ rotated' = BugMarkFutureReanchorRotated
  /\ recoveryOwner' = IF BugPreserveFutureReanchorActiveMarkers THEN recoveryOwner ELSE "None"
  /\ quorumRescheduleArmed' = FALSE
  /\ quorumWindowAge' = 0
  /\ validationState' =
       IF BugPreserveFutureReanchorActiveMarkers THEN validationState ELSE "Pending"
  /\ localVoteEmitted' =
       IF BugPreserveFutureReanchorActiveMarkers THEN localVoteEmitted ELSE FALSE
  /\ commitQcObserved' =
       IF BugPreserveFutureReanchorActiveMarkers THEN commitQcObserved ELSE FALSE
  /\ payloadRecovered' =
       IF BugPreserveFutureReanchorActiveMarkers THEN payloadRecovered ELSE FALSE
  /\ quorumRetransmitted' =
       IF BugPreserveFutureReanchorActiveMarkers THEN quorumRetransmitted ELSE FALSE
  /\ futurePromotionReady' = TRUE
  /\ promotionFresh' = FALSE
  /\ UNCHANGED ViewRecoveryBookkeeping
  /\ UNCHANGED <<
      frontierSlot,
      contiguous,
      committed,
      dropped,
      dropReason,
      commitVotes,
      queuedVotes,
      payloadState,
      view,
      futurePresent,
      futureContiguous,
      futureCommitVotes,
      futureQueuedVotes,
      futurePayloadState,
      futureRecoveryOwner,
      futurePromoted,
      futureEvidenceObserved,
      gst
     >>

PromoteFutureFrontier ==
  /\ PromoteFutureFrontierEnabled
  /\ RecordProgress("Promotion")
  /\ frontierSlot' = frontierSlot + 1
  /\ pending' = TRUE
  /\ contiguous' = futureContiguous
  /\ committed' = FALSE
  /\ dropped' = FALSE
  /\ dropReason' = "None"
  /\ commitVotes' = futureCommitVotes
  /\ queuedVotes' = futureQueuedVotes
  /\ payloadState' = futurePayloadState
  /\ recoveryOwner' = futureRecoveryOwner
  /\ quorumRescheduleArmed' = IF BugPromoteWithoutReset THEN quorumRescheduleArmed ELSE FALSE
  /\ quorumWindowAge' = IF BugPromoteWithoutReset THEN quorumWindowAge ELSE 0
  /\ view' = IF BugPromoteWithoutReset THEN view ELSE 0
  /\ subjectView' = IF BugPromoteWithoutReset THEN subjectView ELSE 0
  /\ validationState' = "Pending"
  /\ localVoteEmitted' = FALSE
  /\ commitQcObserved' = FALSE
  /\ recoveryLastRotationView' = IF BugPromoteWithoutReset THEN recoveryLastRotationView ELSE 0
  /\ staleRecoveryUnlocked' = FALSE
  /\ payloadRecovered' = IF BugPromoteWithoutReset THEN payloadRecovered ELSE FALSE
  /\ quorumRetransmitted' = IF BugPromoteWithoutReset THEN quorumRetransmitted ELSE FALSE
  /\ rotated' = FALSE
  /\ futurePresent' = FALSE
  /\ futureContiguous' = FALSE
  /\ futureCommitVotes' = 0
  /\ futureQueuedVotes' = 0
  /\ futurePayloadState' = "Missing"
  /\ futureRecoveryOwner' = "None"
  /\ futurePromotionReady' = FALSE
  /\ futurePromoted' = TRUE
  /\ futureEvidenceObserved' = TRUE
  /\ promotionFresh' = TRUE
  /\ UNCHANGED gst

DropFutureFrontierEvidence ==
  /\ DropFutureFrontierEvidenceEnabled
  /\ ClearProgressEvent
  /\ futurePresent' = FALSE
  /\ futureContiguous' = FALSE
  /\ futureCommitVotes' = 0
  /\ futureQueuedVotes' = 0
  /\ futurePayloadState' = "Missing"
  /\ futureRecoveryOwner' = "None"
  /\ UNCHANGED PendingProgressFlags
  /\ UNCHANGED ViewRecoveryBookkeeping
  /\ UNCHANGED <<
      frontierSlot,
      pending,
      contiguous,
      committed,
      dropped,
      dropReason,
      commitVotes,
      queuedVotes,
      payloadState,
      recoveryOwner,
      quorumRescheduleArmed,
      quorumWindowAge,
      view,
      payloadRecovered,
      quorumRetransmitted,
      rotated,
      futurePromotionReady,
      futurePromoted,
      futureEvidenceObserved,
      promotionFresh,
      gst
     >>

DropZeroEvidence ==
  /\ DropZeroEvidenceEnabled
  /\ ClearProgressEvent
  /\ pending' = FALSE
  /\ dropped' = TRUE
  /\ dropReason' = "ZeroEvidence"
  /\ quorumRescheduleArmed' = FALSE
  /\ quorumWindowAge' = 0
  /\ promotionFresh' = FALSE
  /\ UNCHANGED PendingProgressFlags
  /\ UNCHANGED ViewRecoveryBookkeeping
  /\ UNCHANGED <<
      frontierSlot,
      contiguous,
      committed,
      commitVotes,
      queuedVotes,
      payloadState,
      recoveryOwner,
      view,
      payloadRecovered,
      quorumRetransmitted,
      rotated,
      futurePresent,
      futureContiguous,
      futureCommitVotes,
      futureQueuedVotes,
      futurePayloadState,
      futureRecoveryOwner,
      futurePromotionReady,
      futurePromoted,
      futureEvidenceObserved,
      gst
     >>

Next ==
  \/ StallBeforeGst
  \/ GstElapsed
  \/ LearnFutureFrontierEvidence
  \/ ValidatePending
  \/ EmitLocalCommitVote
  \/ ObserveCommitQc
  \/ ClearStaleRecovery
  \/ DrainVoteQueue
  \/ RecoverPayload
  \/ CommitFrontier
  \/ ArmQuorumReschedule
  \/ QuorumWindowTick
  \/ QuorumRetransmit
  \/ RotateVoteBackedFrontier
  \/ DropAtViewBound
  \/ ClearCurrentForFutureReanchor
  \/ PromoteFutureFrontier
  \/ DropFutureFrontierEvidence
  \/ DropZeroEvidence

Fairness ==
  /\ WF_vars(GstElapsed)
  /\ WF_vars(ValidatePending)
  /\ WF_vars(EmitLocalCommitVote)
  /\ WF_vars(ObserveCommitQc)
  /\ WF_vars(ClearStaleRecovery)
  /\ WF_vars(DrainVoteQueue)
  /\ WF_vars(RecoverPayload)
  /\ WF_vars(CommitFrontier)
  /\ WF_vars(ArmQuorumReschedule)
  /\ WF_vars(QuorumWindowTick)
  /\ WF_vars(QuorumRetransmit)
  /\ WF_vars(RotateVoteBackedFrontier)
  /\ WF_vars(DropAtViewBound)
  /\ WF_vars(ClearCurrentForFutureReanchor)
  /\ WF_vars(PromoteFutureFrontier)
  /\ WF_vars(DropZeroEvidence)

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ Fairness

SpecTlcFast ==
  /\ Init
  /\ TlcFastCanonicalInit
  /\ [][Next]_vars
  /\ Fairness

CommitImpliesVoteQuorum ==
  committed => commitVotes >= CommitQuorum

CommitImpliesPayloadAvailability ==
  committed => PayloadAvailable

CommittedFrontierHasNoStagedFuture ==
  committed
  => /\ FreshRecoveryOwner
     /\ ~futurePresent
     /\ ~futurePromotionReady
     /\ (futureEvidenceObserved => futurePromoted)

VoteBackedNotDroppedAsZeroEvidenceZombie ==
  dropReason = "ZeroEvidence" => ~VoteBacked

ZeroEvidenceDropHasNoConsensusEvidence ==
  dropReason = "ZeroEvidence"
  => /\ dropped
     /\ ~rotated
     /\ ~VoteBacked
     /\ ~FullQuorum
     /\ ~commitQcObserved
     /\ ~quorumRetransmitted
     /\ ~futurePromotionReady

ZeroEvidenceDropHasNoStagedFuture ==
  dropReason = "ZeroEvidence"
  => /\ ~futurePresent
     /\ ~futurePromotionReady
     /\ (futureEvidenceObserved => futurePromoted)

PostGstVoteBackedFrontierHasProgress ==
  /\ gst
  /\ ActiveFrontier
  /\ VoteBacked
  => FrontierProgressEnabled

FuturePromotionReadyHasProgress ==
  /\ gst
  /\ futurePromotionReady
  => PromoteFutureFrontierEnabled

StaleRecoveryOwnerHasClearProgress ==
  /\ gst
  /\ ActiveFrontier
  /\ recoveryOwner = "Stale"
  /\ StaleRecoveryViewCovered
  => ClearStaleRecoveryEnabled

VoteQueueBacklogHasDrainProgress ==
  /\ gst
  /\ ActiveFrontier
  /\ FreshRecoveryOwner
  /\ ~FutureReanchorReadyForCurrent
  /\ queuedVotes > 0
  => DrainVoteQueueEnabled

MissingPayloadHasRecoveryProgress ==
  /\ gst
  /\ ActiveFrontier
  /\ VoteBacked
  /\ FreshRecoveryOwner
  /\ ~FutureReanchorReadyForCurrent
  /\ queuedVotes = 0
  /\ payloadState = "Missing"
  => RecoverPayloadEnabled

PayloadRecoveredHasLocalOwner ==
  payloadRecovered
  => /\ PayloadAvailable
     /\ recoveryOwner = "Local"

QuorumWindowHasRetransmitProgress ==
  /\ gst
  /\ ActiveFrontier
  /\ VoteBacked
  /\ FreshRecoveryOwner
  /\ ~FutureReanchorReadyForCurrent
  /\ PayloadAvailable
  /\ ~FullQuorum
  /\ quorumRescheduleArmed
  /\ quorumWindowAge >= RescheduleWindow
  => QuorumRetransmitEnabled

QuorumRetransmitClearsRescheduleWindow ==
  quorumRetransmitted
  => /\ ~quorumRescheduleArmed
     /\ quorumWindowAge = 0
     /\ VoteBacked
     /\ PayloadAvailable

RetransmitHasFollowthroughProgress ==
  /\ gst
  /\ ActiveFrontier
  /\ VoteBacked
  /\ FreshRecoveryOwner
  /\ ~FutureReanchorReadyForCurrent
  /\ PayloadAvailable
  /\ ~FullQuorum
  /\ ~quorumRescheduleArmed
  /\ quorumRetransmitted
  => RotateVoteBackedFrontierEnabled \/ DropAtViewBoundEnabled

FutureEvidenceHasReanchorProgress ==
  /\ gst
  /\ FutureReanchorReadyForCurrent
  /\ ~futurePromotionReady
  => ClearCurrentForFutureReanchorEnabled

FutureEvidencePreservedUntilPromotion ==
  /\ futureEvidenceObserved
  /\ ~futurePromoted
  => FutureFrontierEvidence

FuturePromotionResetsActiveProgress ==
  promotionFresh
  => /\ ~payloadRecovered
     /\ ~quorumRetransmitted
     /\ ~quorumRescheduleArmed
     /\ quorumWindowAge = 0
     /\ view = 0
     /\ subjectView = view
     /\ progressAge = 0
     /\ validationState = "Pending"
     /\ ~localVoteEmitted
     /\ ~commitQcObserved
     /\ recoveryLastRotationView = view
     /\ ~staleRecoveryUnlocked

FuturePromotionInstallsFreshSecondSlot ==
  promotionFresh
  => /\ PromotedSecondSlotPending
     /\ dropReason = "None"
     /\ futurePromoted
     /\ futureEvidenceObserved
     /\ ~futurePresent
     /\ ~futureContiguous
     /\ futureCommitVotes = 0
     /\ futureQueuedVotes = 0
     /\ futurePayloadState = "Missing"
     /\ futureRecoveryOwner = "None"
     /\ ~futurePromotionReady
     /\ FuturePromotionResetsActiveProgress

FuturePromotionReadyClearsCurrentWrapper ==
  futurePromotionReady
  => /\ frontierSlot = 0
     /\ ~pending
     /\ ~committed
     /\ ~dropped
     /\ ~rotated
     /\ dropReason = "None"
     /\ ~quorumRescheduleArmed
     /\ quorumWindowAge = 0
     /\ futurePresent
     /\ futureContiguous
     /\ FutureVoteBacked
     /\ futureEvidenceObserved
     /\ ~futurePromoted
     /\ lastProgressKind = "FutureReanchor"
     /\ progressAge = 0

FuturePromotionReadyClearsActiveMarkers ==
  futurePromotionReady
  => /\ recoveryOwner = "None"
     /\ validationState = "Pending"
     /\ ~localVoteEmitted
     /\ ~commitQcObserved
     /\ ~payloadRecovered
     /\ ~quorumRetransmitted

TerminalFrontierOutcomesAreExclusive ==
  /\ committed =>
       /\ ~pending
       /\ ~dropped
       /\ dropReason = "None"
       /\ ~rotated
       /\ ~quorumRescheduleArmed
       /\ quorumWindowAge = 0
       /\ ValidationReady
       /\ FullQuorum
       /\ PayloadAvailable
  /\ dropped =>
       /\ ~pending
       /\ ~committed
       /\ dropReason # "None"
       /\ ~quorumRescheduleArmed
       /\ quorumWindowAge = 0
  /\ dropReason # "None" => dropped
  /\ rotated /\ ~dropped =>
       /\ ~pending
       /\ ~committed
       /\ dropReason = "None"
       /\ ~quorumRescheduleArmed
       /\ quorumWindowAge = 0

RotatedFrontierHasRetransmitEvidence ==
  rotated /\ ~dropped
  => /\ ~committed
     /\ dropReason = "None"
     /\ view > 0
     /\ subjectView = view
     /\ recoveryLastRotationView = view
     /\ ~staleRecoveryUnlocked
     /\ quorumRetransmitted
     /\ VoteBacked
     /\ PayloadAvailable
     /\ FreshRecoveryOwner
     /\ ~FullQuorum

RotatedFrontierHasNoStagedFuture ==
  rotated /\ ~dropped
  => /\ ~futurePresent
     /\ ~futurePromotionReady
     /\ (futureEvidenceObserved => futurePromoted)

ViewBoundDropHasRetransmitEvidence ==
  dropReason = "ViewBound"
  => /\ dropped
     /\ rotated
     /\ view = MaxView
     /\ quorumRetransmitted
     /\ VoteBacked
     /\ PayloadAvailable
     /\ FreshRecoveryOwner
     /\ ~FullQuorum

ViewBoundDropHasNoStagedFuture ==
  dropReason = "ViewBound"
  => /\ ~futurePresent
     /\ ~futurePromotionReady
     /\ (futureEvidenceObserved => futurePromoted)

PendingProgressEventsTouchAge ==
  lastProgressKind # "None" => progressAge = 0

StaleRecoveryUnlockIsViewScoped ==
  staleRecoveryUnlocked => StaleRecoveryViewCovered

StaleRecoveryUnlockClearsStaleOwner ==
  staleRecoveryUnlocked => FreshRecoveryOwner

PostGstVoteBackedFrontierEventuallyResolves ==
  [] (
      /\ gst
      /\ ActiveFrontier
      /\ VoteBacked
      => <> ~pending
     )

RecoveredPayloadEventuallyAdvances ==
  [] (
      /\ gst
      /\ ActiveFrontier
      /\ VoteBacked
      /\ payloadRecovered
      => <> (~pending \/ committed \/ quorumRetransmitted)
     )

QuorumRetransmitEventuallyLeavesPending ==
  [] (
      /\ gst
      /\ ActiveFrontier
      /\ VoteBacked
      /\ quorumRetransmitted
      => <> ~pending
     )

FutureFrontierEvidenceEventuallyReanchors ==
  [] (
      /\ gst
      /\ ActiveFrontier
      /\ FreshRecoveryOwner
      /\ FutureFrontierEvidence
      => <> futurePromoted
     )

FuturePromotionReadyEventuallyPromotes ==
  [] (
      /\ gst
      /\ futurePromotionReady
      => <> futurePromoted
     )

PromotedSecondSlotEventuallyClears ==
  [] (
      /\ gst
      /\ PromotedSecondSlotPending
      => <> ~pending
     )

====
