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
  \* @type: Bool;
  BugDisableStaleRecovery,
  \* @type: Bool;
  BugDisableQueueDrain,
  \* @type: Bool;
  BugDisablePayloadRecovery,
  \* @type: Bool;
  BugDisableRetransmitFollowthrough,
  \* @type: Bool;
  BugDisableFuturePromotion,
  \* @type: Bool;
  BugDisableFutureReanchorClear,
  \* @type: Bool;
  BugAllowFutureEvidenceDrop,
  \* @type: Bool;
  BugPromoteWithoutReset,
  \* @type: Bool;
  BugFutureStaleOwnerBlocksReanchor

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

ActiveFrontier ==
  /\ pending
  /\ contiguous
  /\ ~committed
  /\ ~dropped

VoteBacked == commitVotes + queuedVotes > 0
FullQuorum == commitVotes >= CommitQuorum
PayloadAvailable == payloadState = "Local"
FreshRecoveryOwner == recoveryOwner # "Stale"

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
  /\ BugDisableStaleRecovery \in BOOLEAN
  /\ BugDisableQueueDrain \in BOOLEAN
  /\ BugDisablePayloadRecovery \in BOOLEAN
  /\ BugDisableRetransmitFollowthrough \in BOOLEAN
  /\ BugDisableFuturePromotion \in BOOLEAN
  /\ BugDisableFutureReanchorClear \in BOOLEAN
  /\ BugAllowFutureEvidenceDrop \in BOOLEAN
  /\ BugPromoteWithoutReset \in BOOLEAN
  /\ BugFutureStaleOwnerBlocksReanchor \in BOOLEAN
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
  /\ view = 0
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

ClearStaleRecoveryEnabled ==
  /\ ~BugDisableStaleRecovery
  /\ ~(BugFutureStaleOwnerBlocksReanchor /\ FutureFrontierEvidence)
  /\ gst
  /\ ActiveFrontier
  /\ recoveryOwner = "Stale"

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
  /\ futurePresent' = TRUE
  /\ futureContiguous' = TRUE
  /\ futureCommitVotes' = 1
  /\ futureQueuedVotes' = 0
  /\ futurePayloadState' = "Missing"
  /\ futureRecoveryOwner' = "None"
  /\ futureEvidenceObserved' = TRUE
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

ClearStaleRecovery ==
  /\ ClearStaleRecoveryEnabled
  /\ recoveryOwner' = "None"
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

DrainVoteQueue ==
  /\ DrainVoteQueueEnabled
  /\ commitVotes' = commitVotes + 1
  /\ queuedVotes' = queuedVotes - 1
  /\ promotionFresh' = FALSE
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
  /\ payloadState' = "Local"
  /\ payloadRecovered' = TRUE
  /\ recoveryOwner' = "Local"
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
  /\ committed' = TRUE
  /\ pending' = FALSE
  /\ quorumRescheduleArmed' = FALSE
  /\ quorumWindowAge' = 0
  /\ promotionFresh' = FALSE
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
  /\ quorumRescheduleArmed' = TRUE
  /\ quorumWindowAge' = 0
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
  /\ quorumWindowAge' = quorumWindowAge + 1
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
  /\ quorumRetransmitted' = TRUE
  /\ quorumRescheduleArmed' = FALSE
  /\ quorumWindowAge' = 0
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
  /\ view' = view + 1
  /\ rotated' = TRUE
  /\ pending' = FALSE
  /\ quorumRescheduleArmed' = FALSE
  /\ quorumWindowAge' = 0
  /\ promotionFresh' = FALSE
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
  /\ pending' = FALSE
  /\ dropped' = TRUE
  /\ dropReason' = "ViewBound"
  /\ rotated' = TRUE
  /\ quorumRescheduleArmed' = FALSE
  /\ quorumWindowAge' = 0
  /\ promotionFresh' = FALSE
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
  /\ pending' = FALSE
  /\ quorumRescheduleArmed' = FALSE
  /\ quorumWindowAge' = 0
  /\ futurePromotionReady' = TRUE
  /\ promotionFresh' = FALSE
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
      futurePromoted,
      futureEvidenceObserved,
      gst
     >>

PromoteFutureFrontier ==
  /\ PromoteFutureFrontierEnabled
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
  /\ futurePresent' = FALSE
  /\ futureContiguous' = FALSE
  /\ futureCommitVotes' = 0
  /\ futureQueuedVotes' = 0
  /\ futurePayloadState' = "Missing"
  /\ futureRecoveryOwner' = "None"
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
  /\ pending' = FALSE
  /\ dropped' = TRUE
  /\ dropReason' = "ZeroEvidence"
  /\ quorumRescheduleArmed' = FALSE
  /\ quorumWindowAge' = 0
  /\ promotionFresh' = FALSE
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

CommitImpliesVoteQuorum ==
  committed => commitVotes >= CommitQuorum

CommitImpliesPayloadAvailability ==
  committed => PayloadAvailable

VoteBackedNotDroppedAsZeroEvidenceZombie ==
  dropReason = "ZeroEvidence" => ~VoteBacked

PostGstVoteBackedFrontierHasProgress ==
  /\ gst
  /\ ActiveFrontier
  /\ VoteBacked
  => FrontierProgressEnabled

FuturePromotionReadyHasProgress ==
  /\ gst
  /\ futurePromotionReady
  => PromoteFutureFrontierEnabled

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
