---- MODULE SumeragiFrontierRecovery ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for the Sumeragi/Taira frontier recovery hang class.

This model intentionally abstracts signatures and ECDSA into finite vote
evidence counters. It focuses on one pending contiguous frontier block and the
state that can otherwise pace or hide recovery:
- queued commit votes that have not reached the local quorum counter yet,
- local payload availability,
- stale vs. fresh frontier recovery ownership,
- quorum-reschedule marker/window state,
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
  BugDisableQueueDrain

VARIABLES
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
  futureFrontierEvidence,
  \* @type: Bool;
  futureFrontierResolved,
  \* @type: Bool;
  gst

vars == <<
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
  futureFrontierEvidence,
  futureFrontierResolved,
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
ResolutionReached == committed \/ payloadRecovered \/ quorumRetransmitted \/ rotated \/ futureFrontierResolved

TypeInvariant ==
  /\ N \in Nat
  /\ CommitQuorum \in 1..N
  /\ MaxBacklog \in 0..N
  /\ MaxView \in Nat
  /\ RescheduleWindow \in Nat
  /\ BugDisableStaleRecovery \in BOOLEAN
  /\ BugDisableQueueDrain \in BOOLEAN
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
  /\ futureFrontierEvidence \in BOOLEAN
  /\ futureFrontierResolved \in BOOLEAN
  /\ futureFrontierResolved => rotated
  /\ committed => ~pending
  /\ rotated => ~pending
  /\ gst \in BOOLEAN

Init ==
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
  /\ futureFrontierEvidence \in BOOLEAN
  /\ futureFrontierResolved = FALSE
  /\ gst = FALSE

ClearStaleRecoveryEnabled ==
  /\ ~BugDisableStaleRecovery
  /\ gst
  /\ ActiveFrontier
  /\ recoveryOwner = "Stale"

DrainVoteQueueEnabled ==
  /\ ~BugDisableQueueDrain
  /\ gst
  /\ ActiveFrontier
  /\ FreshRecoveryOwner
  /\ queuedVotes > 0

RecoverPayloadEnabled ==
  /\ gst
  /\ ActiveFrontier
  /\ FreshRecoveryOwner
  /\ queuedVotes = 0
  /\ payloadState = "Missing"

CommitFrontierEnabled ==
  /\ gst
  /\ ActiveFrontier
  /\ FreshRecoveryOwner
  /\ PayloadAvailable
  /\ FullQuorum

ArmQuorumRescheduleEnabled ==
  /\ gst
  /\ ActiveFrontier
  /\ VoteBacked
  /\ FreshRecoveryOwner
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
  /\ PayloadAvailable
  /\ ~FullQuorum
  /\ quorumRescheduleArmed
  /\ quorumWindowAge < RescheduleWindow

QuorumRetransmitEnabled ==
  /\ gst
  /\ ActiveFrontier
  /\ VoteBacked
  /\ FreshRecoveryOwner
  /\ PayloadAvailable
  /\ ~FullQuorum
  /\ quorumRescheduleArmed
  /\ quorumWindowAge >= RescheduleWindow

RotateVoteBackedFrontierEnabled ==
  /\ gst
  /\ ActiveFrontier
  /\ VoteBacked
  /\ FreshRecoveryOwner
  /\ PayloadAvailable
  /\ ~FullQuorum
  /\ ~quorumRescheduleArmed
  /\ quorumRetransmitted
  /\ view < MaxView

DropAtViewBoundEnabled ==
  /\ gst
  /\ ActiveFrontier
  /\ VoteBacked
  /\ FreshRecoveryOwner
  /\ PayloadAvailable
  /\ ~FullQuorum
  /\ ~quorumRescheduleArmed
  /\ quorumRetransmitted
  /\ view = MaxView

AdoptFutureFrontierEvidenceEnabled ==
  /\ gst
  /\ ActiveFrontier
  /\ VoteBacked
  /\ FreshRecoveryOwner
  /\ futureFrontierEvidence
  /\ ~futureFrontierResolved

DropZeroEvidenceEnabled ==
  /\ gst
  /\ ActiveFrontier
  /\ ~VoteBacked

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
  \/ AdoptFutureFrontierEvidenceEnabled

StallBeforeGst ==
  /\ ~gst
  /\ UNCHANGED vars

GstElapsed ==
  /\ ~gst
  /\ gst' = TRUE
  /\ UNCHANGED <<
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
      futureFrontierEvidence,
      futureFrontierResolved
     >>

ClearStaleRecovery ==
  /\ ClearStaleRecoveryEnabled
  /\ recoveryOwner' = "None"
  /\ UNCHANGED <<
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
      futureFrontierEvidence,
      futureFrontierResolved,
      gst
     >>

DrainVoteQueue ==
  /\ DrainVoteQueueEnabled
  /\ commitVotes' = commitVotes + 1
  /\ queuedVotes' = queuedVotes - 1
  /\ UNCHANGED <<
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
      futureFrontierEvidence,
      futureFrontierResolved,
      gst
     >>

RecoverPayload ==
  /\ RecoverPayloadEnabled
  /\ payloadState' = "Local"
  /\ payloadRecovered' = TRUE
  /\ recoveryOwner' = "Local"
  /\ UNCHANGED <<
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
      futureFrontierEvidence,
      futureFrontierResolved,
      gst
     >>

CommitFrontier ==
  /\ CommitFrontierEnabled
  /\ committed' = TRUE
  /\ pending' = FALSE
  /\ quorumRescheduleArmed' = FALSE
  /\ quorumWindowAge' = 0
  /\ UNCHANGED <<
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
      futureFrontierEvidence,
      futureFrontierResolved,
      gst
     >>

ArmQuorumReschedule ==
  /\ ArmQuorumRescheduleEnabled
  /\ quorumRescheduleArmed' = TRUE
  /\ quorumWindowAge' = 0
  /\ UNCHANGED <<
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
      futureFrontierEvidence,
      futureFrontierResolved,
      gst
     >>

QuorumWindowTick ==
  /\ QuorumWindowTickEnabled
  /\ quorumWindowAge' = quorumWindowAge + 1
  /\ UNCHANGED <<
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
      futureFrontierEvidence,
      futureFrontierResolved,
      gst
     >>

QuorumRetransmit ==
  /\ QuorumRetransmitEnabled
  /\ quorumRetransmitted' = TRUE
  /\ quorumRescheduleArmed' = FALSE
  /\ quorumWindowAge' = 0
  /\ UNCHANGED <<
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
      futureFrontierEvidence,
      futureFrontierResolved,
      gst
     >>

RotateVoteBackedFrontier ==
  /\ RotateVoteBackedFrontierEnabled
  /\ view' = view + 1
  /\ rotated' = TRUE
  /\ pending' = FALSE
  /\ quorumRescheduleArmed' = FALSE
  /\ quorumWindowAge' = 0
  /\ UNCHANGED <<
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
      futureFrontierEvidence,
      futureFrontierResolved,
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
  /\ UNCHANGED <<
      contiguous,
      committed,
      commitVotes,
      queuedVotes,
      payloadState,
      recoveryOwner,
      view,
      payloadRecovered,
      quorumRetransmitted,
      futureFrontierEvidence,
      futureFrontierResolved,
      gst
     >>

AdoptFutureFrontierEvidence ==
  /\ AdoptFutureFrontierEvidenceEnabled
  /\ futureFrontierResolved' = TRUE
  /\ rotated' = TRUE
  /\ pending' = FALSE
  /\ quorumRescheduleArmed' = FALSE
  /\ quorumWindowAge' = 0
  /\ view' = IF view < MaxView THEN view + 1 ELSE view
  /\ UNCHANGED <<
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
      futureFrontierEvidence,
      gst
     >>

DropZeroEvidence ==
  /\ DropZeroEvidenceEnabled
  /\ pending' = FALSE
  /\ dropped' = TRUE
  /\ dropReason' = "ZeroEvidence"
  /\ quorumRescheduleArmed' = FALSE
  /\ quorumWindowAge' = 0
  /\ UNCHANGED <<
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
      futureFrontierEvidence,
      futureFrontierResolved,
      gst
     >>

Next ==
  \/ StallBeforeGst
  \/ GstElapsed
  \/ ClearStaleRecovery
  \/ DrainVoteQueue
  \/ RecoverPayload
  \/ CommitFrontier
  \/ ArmQuorumReschedule
  \/ QuorumWindowTick
  \/ QuorumRetransmit
  \/ RotateVoteBackedFrontier
  \/ DropAtViewBound
  \/ AdoptFutureFrontierEvidence
  \/ DropZeroEvidence

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

PostGstVoteBackedFrontierEventuallyResolves ==
  [] (
      /\ gst
      /\ ActiveFrontier
      /\ VoteBacked
      /\ ~ResolutionReached
      => <> ResolutionReached
     )

RecoveredPayloadEventuallyAdvances ==
  [] (
      /\ gst
      /\ ActiveFrontier
      /\ VoteBacked
      /\ payloadRecovered
      => <> (committed \/ quorumRetransmitted \/ rotated \/ futureFrontierResolved)
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
      /\ VoteBacked
      /\ FreshRecoveryOwner
      /\ futureFrontierEvidence
      => <> (~pending \/ futureFrontierResolved)
     )

====
