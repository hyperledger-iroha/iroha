---- MODULE SumeragiMessageProjectionGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for the remaining consensus-message projection
helpers in `main_loop.rs`.

This slice captures `MessageTimingGuard::for_message(...)`,
`MessageTimingGuard::drop(...)`, `Actor::consensus_control_kind(...)`, and
`Actor::native_amx_message_kind(...)`. BlockCreated and BlockSyncUpdate are
the only block-message variants that produce timing guards; their labels and
header height/view fields must be preserved exactly. Dropping a guard converts
elapsed milliseconds to `u64` with saturation at `u64::MAX`. Control and
native-AMX log labels must remain exact so background diagnostics and status
correlation do not collapse request/vote or prepare/commit traffic.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

\* Reuse the block-message variant universe from the block-message kind gate:
\* 1 BlockCreated, 2 BlockSyncUpdate, 3 FetchBlockBody, 4 BlockBodyResponse,
\* 5..8 CertifiedBlockFetch variants, 9 ConsensusParams, 10..12 QcVote
\* phases, 13..15 Qc phases, 16 VrfCommit, 17 VrfReveal, 18 ExecWitness,
\* 19 RbcInitRequest, 20 RbcChunkRequest, 21 RbcInit, 22 RbcChunk,
\* 23 RbcChunkCompact, 24 RbcReady, 25 RbcDeliver, 26 FetchPendingBlock,
\* 27 KuraReplicaAdvert, 28 ProposalHint, 29 Proposal.
BlockCases == 1..29
TimedBlockCases == {1, 2}

NoTimingLabel == 0
TimingBlockCreatedLabel == 1
TimingBlockSyncUpdateLabel == 2
TimingLabels == 0..2

HeaderHeight(c) ==
  CASE c = 1 -> 7
    [] c = 2 -> 11
    [] OTHER -> 0

HeaderView(c) ==
  CASE c = 1 -> 3
    [] c = 2 -> 5
    [] OTHER -> 0

SpecTimingPresent(c) ==
  c \in TimedBlockCases

SpecTimingLabel(c) ==
  CASE c = 1 -> TimingBlockCreatedLabel
    [] c = 2 -> TimingBlockSyncUpdateLabel
    [] OTHER -> NoTimingLabel

SpecTimingHeight(c) ==
  IF SpecTimingPresent(c) THEN HeaderHeight(c) ELSE 0

SpecTimingView(c) ==
  IF SpecTimingPresent(c) THEN HeaderView(c) ELSE 0

ActualTimingPresent(c) ==
  CASE Bug = "time_all_messages" -> TRUE
    [] Bug = "time_no_messages" -> FALSE
    [] Bug = "time_only_created" -> c = 1
    [] Bug = "time_only_sync" -> c = 2
    [] Bug = "time_proposal" -> c \in {1, 2, 29}
    [] OTHER -> SpecTimingPresent(c)

ActualTimingLabel(c) ==
  CASE Bug = "created_uses_sync_label"
       /\ c = 1 -> TimingBlockSyncUpdateLabel
    [] Bug = "sync_uses_created_label"
       /\ c = 2 -> TimingBlockCreatedLabel
    [] Bug = "proposal_uses_created_label"
       /\ c = 29 -> TimingBlockCreatedLabel
    [] OTHER -> SpecTimingLabel(c)

ActualTimingHeight(c) ==
  CASE Bug = "created_uses_view_as_height"
       /\ c = 1 -> HeaderView(c)
    [] Bug = "sync_height_zero"
       /\ c = 2 -> 0
    [] Bug = "swap_payload_heights"
       /\ c = 1 -> HeaderHeight(2)
    [] Bug = "swap_payload_heights"
       /\ c = 2 -> HeaderHeight(1)
    [] OTHER -> SpecTimingHeight(c)

ActualTimingView(c) ==
  CASE Bug = "created_view_zero"
       /\ c = 1 -> 0
    [] Bug = "sync_uses_height_as_view"
       /\ c = 2 -> HeaderHeight(c)
    [] OTHER -> SpecTimingView(c)

\* The finite u64 abstraction keeps the exact boundary semantics while making
\* expected-failure checks fast.
MaxU64 == 10
ElapsedCases == 0..12

SpecElapsedMs(e) ==
  IF e <= MaxU64 THEN e ELSE MaxU64

ActualElapsedMs(e) ==
  CASE Bug = "elapsed_wraps_overflow"
       /\ e > MaxU64 -> e - (MaxU64 + 1)
    [] Bug = "elapsed_caps_below_max"
       /\ e >= MaxU64 -> MaxU64 - 1
    [] Bug = "elapsed_zeroes_overflow"
       /\ e > MaxU64 -> 0
    [] Bug = "elapsed_off_by_one"
       /\ e > 0
       /\ e < MaxU64 -> e + 1
    [] OTHER -> SpecElapsedMs(e)

ControlEvidenceLabel == 1
ControlUnknownLabel == 0
ControlNativeAmxLabel == 2
ControlLabels == 0..2

SpecControlKind == ControlEvidenceLabel

ActualControlKind ==
  CASE Bug = "control_evidence_unlabeled" -> ControlUnknownLabel
    [] Bug = "control_evidence_native_label" -> ControlNativeAmxLabel
    [] OTHER -> SpecControlKind

AmxCases == 1..4
AmxPrepareRequestLabel == 1
AmxPrepareVoteLabel == 2
AmxCommitRequestLabel == 3
AmxCommitVoteLabel == 4
AmxLabels == 1..4

SpecAmxKind(c) ==
  CASE c = 1 -> AmxPrepareRequestLabel
    [] c = 2 -> AmxPrepareVoteLabel
    [] c = 3 -> AmxCommitRequestLabel
    [] c = 4 -> AmxCommitVoteLabel

ActualAmxKind(c) ==
  CASE Bug = "amx_prepare_request_uses_vote_label"
       /\ c = 1 -> AmxPrepareVoteLabel
    [] Bug = "amx_prepare_vote_uses_commit_vote_label"
       /\ c = 2 -> AmxCommitVoteLabel
    [] Bug = "amx_commit_request_uses_prepare_label"
       /\ c = 3 -> AmxPrepareRequestLabel
    [] Bug = "amx_commit_vote_uses_prepare_vote_label"
       /\ c = 4 -> AmxPrepareVoteLabel
    [] Bug = "amx_collapse_by_phase"
       /\ c = 2 -> AmxPrepareRequestLabel
    [] Bug = "amx_collapse_by_phase"
       /\ c = 4 -> AmxCommitRequestLabel
    [] OTHER -> SpecAmxKind(c)

Init ==
  checked = 0

Next ==
  \/ /\ checked < 29
     /\ checked' = checked + 1
  \/ /\ checked = 29
     /\ UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "time_all_messages",
       "time_no_messages",
       "time_only_created",
       "time_only_sync",
       "time_proposal",
       "created_uses_sync_label",
       "sync_uses_created_label",
       "proposal_uses_created_label",
       "created_uses_view_as_height",
       "sync_height_zero",
       "swap_payload_heights",
       "created_view_zero",
       "sync_uses_height_as_view",
       "elapsed_wraps_overflow",
       "elapsed_caps_below_max",
       "elapsed_zeroes_overflow",
       "elapsed_off_by_one",
       "control_evidence_unlabeled",
       "control_evidence_native_label",
       "amx_prepare_request_uses_vote_label",
       "amx_prepare_vote_uses_commit_vote_label",
       "amx_commit_request_uses_prepare_label",
       "amx_commit_vote_uses_prepare_vote_label",
       "amx_collapse_by_phase"
     }
  /\ checked \in 0..29
  /\ \A c \in BlockCases:
       /\ ActualTimingPresent(c) \in BOOLEAN
       /\ ActualTimingLabel(c) \in TimingLabels
       /\ ActualTimingHeight(c) \in 0..12
       /\ ActualTimingView(c) \in 0..12
  /\ \A e \in ElapsedCases: ActualElapsedMs(e) \in 0..MaxU64
  /\ ActualControlKind \in ControlLabels
  /\ \A c \in AmxCases: ActualAmxKind(c) \in AmxLabels

TimingPresenceExact ==
  \A c \in BlockCases:
    ActualTimingPresent(c) = SpecTimingPresent(c)

TimingLabelsExact ==
  \A c \in BlockCases:
    ActualTimingLabel(c) = SpecTimingLabel(c)

TimingHeaderFieldsExact ==
  \A c \in BlockCases:
    /\ ActualTimingHeight(c) = SpecTimingHeight(c)
    /\ ActualTimingView(c) = SpecTimingView(c)

OnlyBlockPayloadsTimed ==
  \A c \in BlockCases:
    ActualTimingPresent(c) => c \in TimedBlockCases

TimedPayloadsPreserveHeaderFields ==
  /\ ActualTimingLabel(1) = TimingBlockCreatedLabel
  /\ ActualTimingHeight(1) = HeaderHeight(1)
  /\ ActualTimingView(1) = HeaderView(1)
  /\ ActualTimingLabel(2) = TimingBlockSyncUpdateLabel
  /\ ActualTimingHeight(2) = HeaderHeight(2)
  /\ ActualTimingView(2) = HeaderView(2)

ElapsedMsSaturates ==
  \A e \in ElapsedCases:
    ActualElapsedMs(e) = SpecElapsedMs(e)

ElapsedMaxBoundaryPreserved ==
  /\ ActualElapsedMs(MaxU64 - 1) = MaxU64 - 1
  /\ ActualElapsedMs(MaxU64) = MaxU64
  /\ ActualElapsedMs(MaxU64 + 1) = MaxU64

ControlKindExact ==
  ActualControlKind = SpecControlKind

NativeAmxLabelsExact ==
  \A c \in AmxCases:
    ActualAmxKind(c) = SpecAmxKind(c)

NativeAmxRequestVoteAndPhaseLabelsDistinct ==
  /\ ActualAmxKind(1) # ActualAmxKind(2)
  /\ ActualAmxKind(3) # ActualAmxKind(4)
  /\ ActualAmxKind(1) # ActualAmxKind(3)
  /\ ActualAmxKind(2) # ActualAmxKind(4)

TimingProjectionAnchors ==
  /\ TimingPresenceExact
  /\ TimingLabelsExact
  /\ TimingHeaderFieldsExact
  /\ OnlyBlockPayloadsTimed
  /\ TimedPayloadsPreserveHeaderFields
  /\ ActualTimingPresent(1)
  /\ ActualTimingPresent(2)
  /\ ~ActualTimingPresent(29)
  /\ ActualTimingLabel(1) = TimingBlockCreatedLabel
  /\ ActualTimingLabel(2) = TimingBlockSyncUpdateLabel

ElapsedProjectionAnchors ==
  /\ ElapsedMsSaturates
  /\ ElapsedMaxBoundaryPreserved
  /\ ActualElapsedMs(0) = 0
  /\ ActualElapsedMs(MaxU64) = MaxU64
  /\ ActualElapsedMs(MaxU64 + 2) = MaxU64

ControlProjectionAnchors ==
  /\ ControlKindExact
  /\ ActualControlKind = ControlEvidenceLabel

NativeAmxProjectionAnchors ==
  /\ NativeAmxLabelsExact
  /\ NativeAmxRequestVoteAndPhaseLabelsDistinct
  /\ ActualAmxKind(1) = AmxPrepareRequestLabel
  /\ ActualAmxKind(2) = AmxPrepareVoteLabel
  /\ ActualAmxKind(3) = AmxCommitRequestLabel
  /\ ActualAmxKind(4) = AmxCommitVoteLabel

MessageProjectionSafetyAnchors ==
  /\ TimingProjectionAnchors
  /\ ElapsedProjectionAnchors
  /\ ControlProjectionAnchors
  /\ NativeAmxProjectionAnchors

SafetyFast ==
  /\ TimingPresenceExact
  /\ TimingLabelsExact
  /\ TimingHeaderFieldsExact
  /\ OnlyBlockPayloadsTimed
  /\ TimedPayloadsPreserveHeaderFields
  /\ ElapsedMsSaturates
  /\ ElapsedMaxBoundaryPreserved
  /\ ControlKindExact
  /\ NativeAmxLabelsExact
  /\ NativeAmxRequestVoteAndPhaseLabelsDistinct

Safety == MessageProjectionSafetyAnchors

====
