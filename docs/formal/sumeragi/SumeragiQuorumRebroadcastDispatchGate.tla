---- MODULE SumeragiQuorumRebroadcastDispatchGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for the dispatch decisions in
`rebroadcast_pending_block_updates(...)` and
`broadcast_vote_backed_block_sync_update(...)`.

Target selection, pressure scoring, and cooldown arithmetic are covered by
adjacent helper models. This slice pins how those helper results are composed:
local vote emission is gated before retransmit work, relay/no-target/cooldown
and backlog exits are fail-closed, forced vote-backed fanout bypasses cooldown
and target-limit throttles, vote replay always precedes optional payload repair,
commit-QC fetches run only for vote-backed blocks without cached commit QC,
BlockCreated replay requires non-dropped vote-backed pending state, near-quorum
contiguous frontier blocks may additionally send a fitting non-local
BlockSyncUpdate, and any actual work stamps the precommit rebroadcast marker.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

DropPendingNoLocalVote == "DropPendingNoLocalVote"
EmptyTopologyNoLocalVote == "EmptyTopologyNoLocalVote"
LocalVoteEmitted == "LocalVoteEmitted"
RelayBackpressureExit == "RelayBackpressureExit"
NoTargetsExit == "NoTargetsExit"
CooldownExit == "CooldownExit"
BacklogLimitZeroExit == "BacklogLimitZeroExit"
PacedTargetsEmptyExit == "PacedTargetsEmptyExit"
ForceFanoutBypassesCooldown == "ForceFanoutBypassesCooldown"
ForceFanoutBypassesLimit == "ForceFanoutBypassesLimit"
VoteReplayOnly == "VoteReplayOnly"
DropPendingSuppressesPayload == "DropPendingSuppressesPayload"
CachedCommitQcSuppressesMissingFetch == "CachedCommitQcSuppressesMissingFetch"
MissingFetchWithVoteBacking == "MissingFetchWithVoteBacking"
ContiguousNearQuorumBlockSync == "ContiguousNearQuorumBlockSync"
BlockSyncFrameTooLarge == "BlockSyncFrameTooLarge"
BlockSyncLocalOnlyTargets == "BlockSyncLocalOnlyTargets"
BlockSyncNonSyncPayload == "BlockSyncNonSyncPayload"
NonContiguousNoBlockSync == "NonContiguousNoBlockSync"
NotNearQuorumNoBlockSync == "NotNearQuorumNoBlockSync"
BlockCreatedWithVoteBacking == "BlockCreatedWithVoteBacking"
NoObservedBackingNoBlockCreated == "NoObservedBackingNoBlockCreated"
AnyActionMarks == "AnyActionMarks"
NoActionNoMark == "NoActionNoMark"

Cases == {
  DropPendingNoLocalVote,
  EmptyTopologyNoLocalVote,
  LocalVoteEmitted,
  RelayBackpressureExit,
  NoTargetsExit,
  CooldownExit,
  BacklogLimitZeroExit,
  PacedTargetsEmptyExit,
  ForceFanoutBypassesCooldown,
  ForceFanoutBypassesLimit,
  VoteReplayOnly,
  DropPendingSuppressesPayload,
  CachedCommitQcSuppressesMissingFetch,
  MissingFetchWithVoteBacking,
  ContiguousNearQuorumBlockSync,
  BlockSyncFrameTooLarge,
  BlockSyncLocalOnlyTargets,
  BlockSyncNonSyncPayload,
  NonContiguousNoBlockSync,
  NotNearQuorumNoBlockSync,
  BlockCreatedWithVoteBacking,
  NoObservedBackingNoBlockCreated,
  AnyActionMarks,
  NoActionNoMark
}

LocalVoteCases == {
  DropPendingNoLocalVote,
  EmptyTopologyNoLocalVote,
  LocalVoteEmitted
}

FailClosedExitCases == {
  RelayBackpressureExit,
  NoTargetsExit,
  CooldownExit,
  BacklogLimitZeroExit,
  PacedTargetsEmptyExit,
  NoActionNoMark
}

ForceFanoutCases == {
  ForceFanoutBypassesCooldown,
  ForceFanoutBypassesLimit
}

VoteReplayCases == {
  ForceFanoutBypassesCooldown,
  ForceFanoutBypassesLimit,
  VoteReplayOnly,
  CachedCommitQcSuppressesMissingFetch,
  MissingFetchWithVoteBacking,
  ContiguousNearQuorumBlockSync,
  BlockCreatedWithVoteBacking,
  AnyActionMarks
}

PayloadDispatchCases == {
  DropPendingSuppressesPayload,
  CachedCommitQcSuppressesMissingFetch,
  MissingFetchWithVoteBacking,
  BlockCreatedWithVoteBacking,
  NoObservedBackingNoBlockCreated
}

BlockSyncDispatchCases == {
  ContiguousNearQuorumBlockSync,
  BlockSyncFrameTooLarge,
  BlockSyncLocalOnlyTargets,
  BlockSyncNonSyncPayload,
  NonContiguousNoBlockSync,
  NotNearQuorumNoBlockSync
}

MarkingCases == {
  AnyActionMarks,
  NoActionNoMark
}

BoolToInt(b) == IF b THEN 1 ELSE 0
Min(a, b) == IF a <= b THEN a ELSE b

MinVotes(c) == 3

DropPending(c) ==
  c \in {DropPendingNoLocalVote, VoteReplayOnly, DropPendingSuppressesPayload}

TopologyNonEmpty(c) ==
  c /= EmptyTopologyNoLocalVote

LocalVoteCanEmit(c) ==
  c \in {DropPendingNoLocalVote, EmptyTopologyNoLocalVote, LocalVoteEmitted}

SpecLocalVote(c) ==
  /\ ~DropPending(c)
  /\ TopologyNonEmpty(c)
  /\ LocalVoteCanEmit(c)

RawVoteCount(c) ==
  CASE c \in {LocalVoteEmitted, NoObservedBackingNoBlockCreated, NoActionNoMark} -> 0
    [] c = NotNearQuorumNoBlockSync -> 1
    [] OTHER -> 2

VoteCountInput(c) ==
  IF c = NoObservedBackingNoBlockCreated THEN 0 ELSE RawVoteCount(c)

EffectiveVoteCount(c) ==
  LET local_floor == IF SpecLocalVote(c) THEN 1 ELSE 0 IN
  LET from_pending == IF RawVoteCount(c) = 0 /\ local_floor = 1 THEN 1 ELSE RawVoteCount(c) IN
  IF from_pending >= VoteCountInput(c) THEN from_pending ELSE VoteCountInput(c)

SpecObservedVoteBacking(c) ==
  EffectiveVoteCount(c) > 0

RelayBackpressure(c) ==
  c = RelayBackpressureExit

InitialTargets(c) ==
  CASE c \in {EmptyTopologyNoLocalVote, NoTargetsExit, NoActionNoMark} -> 0
    [] c = BlockSyncLocalOnlyTargets -> 1
    [] OTHER -> 2

WidenRepairFanout(c) ==
  c \in {ForceFanoutBypassesCooldown, ForceFanoutBypassesLimit}

SpecForceFullFanout(c) ==
  /\ WidenRepairFanout(c)
  /\ ~DropPending(c)
  /\ SpecObservedVoteBacking(c)
  /\ EffectiveVoteCount(c) < MinVotes(c)

CooldownDue(c) ==
  c \notin {CooldownExit, ForceFanoutBypassesCooldown}

TargetLimit(c) ==
  CASE c \in {BacklogLimitZeroExit, ForceFanoutBypassesLimit} -> 0
    [] OTHER -> 1

PacedTargetCount(c) ==
  CASE c = PacedTargetsEmptyExit -> 0
    [] TargetLimit(c) = 0 -> 0
    [] OTHER -> Min(InitialTargets(c), TargetLimit(c))

SpecTargetCount(c) ==
  IF SpecForceFullFanout(c) THEN 3 ELSE PacedTargetCount(c)

NonLocalTargetCount(c) ==
  IF c = BlockSyncLocalOnlyTargets THEN 0 ELSE SpecTargetCount(c)

VotesSentByReplay(c) ==
  IF SpecTargetCount(c) = 0 THEN 0 ELSE SpecTargetCount(c)

HasCachedCommitQc(c) ==
  c = CachedCommitQcSuppressesMissingFetch

ContiguousFrontier(c) ==
  c \notin {NonContiguousNoBlockSync}

NearCommitQuorum(c) ==
  /\ EffectiveVoteCount(c) < MinVotes(c)
  /\ EffectiveVoteCount(c) + 1 >= MinVotes(c)
  /\ c /= NotNearQuorumNoBlockSync

BuildsBlockSyncUpdate(c) ==
  c /= BlockSyncNonSyncPayload

BlockSyncFitsFrame(c) ==
  c /= BlockSyncFrameTooLarge

SpecBlockSyncBroadcast(c) ==
  /\ SpecTargetCount(c) > 0
  /\ NonLocalTargetCount(c) > 0
  /\ BuildsBlockSyncUpdate(c)
  /\ BlockSyncFitsFrame(c)

KnownEvidenceReplay(c) ==
  FALSE

SpecVotes(c) ==
  IF c \in {
       RelayBackpressureExit,
       NoTargetsExit,
       CooldownExit,
       BacklogLimitZeroExit,
       PacedTargetsEmptyExit,
       NoActionNoMark
     } THEN 0
  ELSE VotesSentByReplay(c)

SpecMissingBlockFetch(c) ==
  /\ ~DropPending(c)
  /\ SpecTargetCount(c) > 0
  /\ ~HasCachedCommitQc(c)
  /\ SpecObservedVoteBacking(c)

SpecBlockCreated(c) ==
  /\ ~DropPending(c)
  /\ SpecTargetCount(c) > 0
  /\ SpecObservedVoteBacking(c)

SpecBlockSync(c) ==
  IF KnownEvidenceReplay(c) THEN TRUE
  ELSE
    /\ ~DropPending(c)
    /\ SpecTargetCount(c) > 0
    /\ SpecObservedVoteBacking(c)
    /\ ContiguousFrontier(c)
    /\ NearCommitQuorum(c)
    /\ SpecBlockSyncBroadcast(c)

SpecEarlyExit(c) ==
  \/ RelayBackpressure(c)
  \/ InitialTargets(c) = 0
  \/ (~SpecForceFullFanout(c) /\ ~CooldownDue(c))
  \/ (~SpecForceFullFanout(c) /\ TargetLimit(c) = 0)
  \/ SpecTargetCount(c) = 0

SpecLocalVoteOut(c) ==
  SpecLocalVote(c)

SpecVotesOut(c) ==
  IF SpecEarlyExit(c) THEN 0 ELSE SpecVotes(c)

SpecBlockSyncOut(c) ==
  IF SpecEarlyExit(c) THEN FALSE ELSE SpecBlockSync(c)

SpecBlockCreatedOut(c) ==
  IF SpecEarlyExit(c) THEN FALSE ELSE SpecBlockCreated(c)

SpecMissingFetchOut(c) ==
  IF SpecEarlyExit(c) THEN FALSE ELSE SpecMissingBlockFetch(c)

SpecMarkRebroadcast(c) ==
  IF SpecEarlyExit(c) THEN FALSE
  ELSE
    \/ SpecLocalVoteOut(c)
    \/ SpecVotesOut(c) > 0
    \/ SpecBlockSyncOut(c)
    \/ SpecBlockCreatedOut(c)
    \/ SpecMissingFetchOut(c)

\* @type: (Str) => <<Int, Int, Int, Int, Int, Int, Int>>;
SpecOutput(c) ==
  <<BoolToInt(SpecLocalVoteOut(c)), SpecVotesOut(c),
    BoolToInt(SpecBlockSyncOut(c)), BoolToInt(SpecBlockCreatedOut(c)),
    BoolToInt(SpecMissingFetchOut(c)), BoolToInt(SpecMarkRebroadcast(c)),
    SpecTargetCount(c)>>

ActualLocalVote(c) ==
  CASE Bug = "local_vote_when_drop" /\ c = DropPendingNoLocalVote -> TRUE
    [] Bug = "local_vote_with_empty_topology" /\ c = EmptyTopologyNoLocalVote ->
       TRUE
    [] OTHER -> SpecLocalVote(c)

ActualObservedVoteBacking(c) ==
  IF Bug = "local_vote_not_counted" /\ c = LocalVoteEmitted
  THEN FALSE
  ELSE
    LET local_floor == IF ActualLocalVote(c) THEN 1 ELSE 0 IN
    LET from_pending == IF RawVoteCount(c) = 0 /\ local_floor = 1 THEN 1 ELSE RawVoteCount(c) IN
    (IF from_pending >= VoteCountInput(c) THEN from_pending ELSE VoteCountInput(c)) > 0

ActualForceFullFanout(c) ==
  CASE Bug = "force_fanout_obeys_cooldown" /\ c = ForceFanoutBypassesCooldown ->
       FALSE
    [] Bug = "force_fanout_obeys_limit" /\ c = ForceFanoutBypassesLimit ->
       FALSE
    [] OTHER ->
       /\ WidenRepairFanout(c)
       /\ ~DropPending(c)
       /\ ActualObservedVoteBacking(c)
       /\ EffectiveVoteCount(c) < MinVotes(c)

ActualTargetCount(c) ==
  CASE Bug = "no_targets_rebroadcasts" /\ c = NoTargetsExit -> 1
    [] Bug = "backlog_limit_zero_ignored" /\ c = BacklogLimitZeroExit ->
       InitialTargets(c)
    [] Bug = "paced_empty_rebroadcasts" /\ c = PacedTargetsEmptyExit ->
       InitialTargets(c)
    [] ActualForceFullFanout(c) -> 3
    [] OTHER -> PacedTargetCount(c)

ActualEarlyExit(c) ==
  CASE Bug = "relay_allows_rebroadcast" /\ c = RelayBackpressureExit -> FALSE
    [] Bug = "no_targets_rebroadcasts" /\ c = NoTargetsExit -> FALSE
    [] Bug = "cooldown_ignored" /\ c = CooldownExit -> FALSE
    [] Bug = "backlog_limit_zero_ignored" /\ c = BacklogLimitZeroExit -> FALSE
    [] Bug = "paced_empty_rebroadcasts" /\ c = PacedTargetsEmptyExit -> FALSE
    [] OTHER ->
       \/ RelayBackpressure(c)
       \/ InitialTargets(c) = 0
       \/ (~ActualForceFullFanout(c) /\ ~CooldownDue(c))
       \/ (~ActualForceFullFanout(c) /\ TargetLimit(c) = 0)
       \/ ActualTargetCount(c) = 0

ActualVotes(c) ==
  CASE Bug = "votes_not_replayed" /\ c = VoteReplayOnly -> 0
    [] ActualTargetCount(c) = 0 -> 0
    [] OTHER -> ActualTargetCount(c)

ActualMissingFetch(c) ==
  CASE Bug = "cached_qc_fetches_missing"
       /\ c = CachedCommitQcSuppressesMissingFetch -> TRUE
    [] Bug = "missing_qc_fetch_skipped"
       /\ c = MissingFetchWithVoteBacking -> FALSE
    [] OTHER ->
       /\ ~DropPending(c)
       /\ ActualTargetCount(c) > 0
       /\ ~HasCachedCommitQc(c)
       /\ ActualObservedVoteBacking(c)

ActualBlockCreated(c) ==
  CASE Bug = "drop_pending_sends_payload"
       /\ c = DropPendingSuppressesPayload -> TRUE
    [] Bug = "block_created_skipped"
       /\ c = BlockCreatedWithVoteBacking -> FALSE
    [] Bug = "observed_backing_ignored_for_block"
       /\ c = NoObservedBackingNoBlockCreated -> TRUE
    [] OTHER ->
       /\ ~DropPending(c)
       /\ ActualTargetCount(c) > 0
       /\ ActualObservedVoteBacking(c)

ActualBlockSyncBroadcast(c) ==
  CASE Bug = "frame_cap_ignored"
       /\ c = BlockSyncFrameTooLarge -> TRUE
    [] Bug = "local_only_block_sync_sent"
       /\ c = BlockSyncLocalOnlyTargets -> TRUE
    [] Bug = "non_sync_payload_sent"
       /\ c = BlockSyncNonSyncPayload -> TRUE
    [] OTHER ->
       /\ ActualTargetCount(c) > 0
       /\ NonLocalTargetCount(c) > 0
       /\ BuildsBlockSyncUpdate(c)
       /\ BlockSyncFitsFrame(c)

ActualBlockSync(c) ==
  CASE Bug = "near_quorum_block_sync_skipped"
       /\ c = ContiguousNearQuorumBlockSync -> FALSE
    [] Bug = "noncontiguous_block_sync_sent"
       /\ c = NonContiguousNoBlockSync -> TRUE
    [] Bug = "not_near_quorum_block_sync_sent"
       /\ c = NotNearQuorumNoBlockSync -> TRUE
    [] OTHER ->
       /\ ~DropPending(c)
       /\ ActualTargetCount(c) > 0
       /\ ActualObservedVoteBacking(c)
       /\ ContiguousFrontier(c)
       /\ NearCommitQuorum(c)
       /\ ActualBlockSyncBroadcast(c)

ActualVotesOut(c) ==
  IF ActualEarlyExit(c) THEN 0 ELSE ActualVotes(c)

ActualBlockSyncOut(c) ==
  IF ActualEarlyExit(c) THEN FALSE ELSE ActualBlockSync(c)

ActualBlockCreatedOut(c) ==
  IF ActualEarlyExit(c) THEN FALSE ELSE ActualBlockCreated(c)

ActualMissingFetchOut(c) ==
  IF ActualEarlyExit(c) THEN FALSE ELSE ActualMissingFetch(c)

ActualMarkRebroadcast(c) ==
  CASE Bug = "mark_skipped_after_action" /\ c = AnyActionMarks -> FALSE
    [] Bug = "mark_without_action" /\ c = NoActionNoMark -> TRUE
    [] ActualEarlyExit(c) -> FALSE
    [] OTHER ->
       \/ ActualLocalVote(c)
       \/ ActualVotesOut(c) > 0
       \/ ActualBlockSyncOut(c)
       \/ ActualBlockCreatedOut(c)
       \/ ActualMissingFetchOut(c)

\* @type: (Str) => <<Int, Int, Int, Int, Int, Int, Int>>;
ActualOutput(c) ==
  <<BoolToInt(ActualLocalVote(c)), ActualVotesOut(c),
    BoolToInt(ActualBlockSyncOut(c)), BoolToInt(ActualBlockCreatedOut(c)),
    BoolToInt(ActualMissingFetchOut(c)), BoolToInt(ActualMarkRebroadcast(c)),
    ActualTargetCount(c)>>

BugSet == {
  "none",
  "local_vote_when_drop",
  "local_vote_with_empty_topology",
  "local_vote_not_counted",
  "relay_allows_rebroadcast",
  "no_targets_rebroadcasts",
  "cooldown_ignored",
  "backlog_limit_zero_ignored",
  "force_fanout_obeys_cooldown",
  "force_fanout_obeys_limit",
  "paced_empty_rebroadcasts",
  "votes_not_replayed",
  "drop_pending_sends_payload",
  "cached_qc_fetches_missing",
  "missing_qc_fetch_skipped",
  "near_quorum_block_sync_skipped",
  "noncontiguous_block_sync_sent",
  "not_near_quorum_block_sync_sent",
  "frame_cap_ignored",
  "local_only_block_sync_sent",
  "non_sync_payload_sent",
  "observed_backing_ignored_for_block",
  "block_created_skipped",
  "mark_skipped_after_action",
  "mark_without_action"
}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in BugSet
  /\ checked = 0

SelectionExact ==
  \A c \in Cases:
    ActualOutput(c) = SpecOutput(c)

ObservedBackingStable ==
  /\ SpecBlockCreatedOut(NoObservedBackingNoBlockCreated) = FALSE
  /\ ActualBlockCreatedOut(NoObservedBackingNoBlockCreated)
     = SpecBlockCreatedOut(NoObservedBackingNoBlockCreated)

EarlyExitStable ==
  /\ SpecOutput(RelayBackpressureExit)[2] = 0
  /\ SpecOutput(RelayBackpressureExit)[3] = 0
  /\ SpecOutput(NoTargetsExit)[2] = 0
  /\ SpecOutput(CooldownExit)[2] = 0
  /\ SpecOutput(BacklogLimitZeroExit)[2] = 0
  /\ SpecOutput(PacedTargetsEmptyExit)[2] = 0
  /\ SpecOutput(NoActionNoMark)[6] = 0

FanoutAndVoteStable ==
  /\ SpecOutput(DropPendingNoLocalVote)[1] = 0
  /\ SpecOutput(EmptyTopologyNoLocalVote)[1] = 0
  /\ SpecOutput(LocalVoteEmitted)[1] = 1
  /\ SpecObservedVoteBacking(LocalVoteEmitted)
  /\ SpecTargetCount(ForceFanoutBypassesCooldown) = 3
  /\ SpecTargetCount(ForceFanoutBypassesLimit) = 3
  /\ SpecOutput(VoteReplayOnly)[2] > 0

PayloadDispatchStable ==
  /\ SpecOutput(DropPendingSuppressesPayload)[3] = 0
  /\ SpecOutput(DropPendingSuppressesPayload)[4] = 0
  /\ SpecOutput(DropPendingSuppressesPayload)[5] = 0
  /\ SpecOutput(CachedCommitQcSuppressesMissingFetch)[5] = 0
  /\ SpecOutput(MissingFetchWithVoteBacking)[5] = 1
  /\ SpecOutput(BlockCreatedWithVoteBacking)[4] = 1
  /\ SpecOutput(NoObservedBackingNoBlockCreated)[4] = 0

BlockSyncDispatchStable ==
  /\ SpecOutput(ContiguousNearQuorumBlockSync)[3] = 1
  /\ SpecOutput(BlockSyncFrameTooLarge)[3] = 0
  /\ SpecOutput(BlockSyncLocalOnlyTargets)[3] = 0
  /\ SpecOutput(BlockSyncNonSyncPayload)[3] = 0
  /\ SpecOutput(NonContiguousNoBlockSync)[3] = 0
  /\ SpecOutput(NotNearQuorumNoBlockSync)[3] = 0

MarkingStable ==
  /\ SpecOutput(AnyActionMarks)[6] = 1
  /\ SpecOutput(NoActionNoMark)[6] = 0

QuorumRebroadcastLocalVoteExact ==
  \A c \in LocalVoteCases:
    /\ ActualLocalVote(c) = SpecLocalVote(c)
    /\ ActualOutput(c)[1] = BoolToInt(SpecLocalVoteOut(c))
    /\ ActualOutput(c) = SpecOutput(c)

QuorumRebroadcastFailClosedExitExact ==
  \A c \in FailClosedExitCases:
    /\ SpecEarlyExit(c)
    /\ ActualEarlyExit(c) = SpecEarlyExit(c)
    /\ ActualVotesOut(c) = 0
    /\ ActualBlockSyncOut(c) = FALSE
    /\ ActualBlockCreatedOut(c) = FALSE
    /\ ActualMissingFetchOut(c) = FALSE
    /\ ActualMarkRebroadcast(c) = FALSE
    /\ ActualOutput(c) = SpecOutput(c)

QuorumRebroadcastForceFanoutExact ==
  \A c \in ForceFanoutCases:
    /\ SpecForceFullFanout(c)
    /\ ActualForceFullFanout(c) = SpecForceFullFanout(c)
    /\ ActualTargetCount(c) = SpecTargetCount(c)
    /\ ActualTargetCount(c) = 3
    /\ ActualEarlyExit(c) = FALSE
    /\ ActualOutput(c) = SpecOutput(c)

QuorumRebroadcastVoteReplayExact ==
  \A c \in VoteReplayCases:
    /\ ~SpecEarlyExit(c)
    /\ ActualVotesOut(c) = SpecVotesOut(c)
    /\ ActualVotesOut(c) = ActualTargetCount(c)
    /\ ActualOutput(c)[2] = SpecOutput(c)[2]
    /\ ActualOutput(c) = SpecOutput(c)

QuorumRebroadcastPayloadDispatchExact ==
  \A c \in PayloadDispatchCases:
    /\ ActualObservedVoteBacking(c) = SpecObservedVoteBacking(c)
    /\ ActualBlockCreatedOut(c) = SpecBlockCreatedOut(c)
    /\ ActualMissingFetchOut(c) = SpecMissingFetchOut(c)
    /\ ActualOutput(c)[4] = SpecOutput(c)[4]
    /\ ActualOutput(c)[5] = SpecOutput(c)[5]
    /\ ActualOutput(c) = SpecOutput(c)

QuorumRebroadcastBlockSyncDispatchExact ==
  \A c \in BlockSyncDispatchCases:
    /\ ActualBlockSyncBroadcast(c) = SpecBlockSyncBroadcast(c)
    /\ ActualBlockSyncOut(c) = SpecBlockSyncOut(c)
    /\ ActualOutput(c)[3] = SpecOutput(c)[3]
    /\ ActualOutput(c) = SpecOutput(c)

QuorumRebroadcastMarkingExact ==
  \A c \in MarkingCases:
    /\ ActualMarkRebroadcast(c) = SpecMarkRebroadcast(c)
    /\ ActualOutput(c)[6] = BoolToInt(SpecMarkRebroadcast(c))
    /\ ActualOutput(c) = SpecOutput(c)

QuorumRebroadcastCoreSafety ==
  /\ SelectionExact
  /\ EarlyExitStable
  /\ FanoutAndVoteStable
  /\ PayloadDispatchStable
  /\ BlockSyncDispatchStable
  /\ MarkingStable

SafetyFast ==
  QuorumRebroadcastCoreSafety

QuorumRebroadcastDispatchExactness ==
  /\ QuorumRebroadcastCoreSafety
  /\ QuorumRebroadcastLocalVoteExact
  /\ QuorumRebroadcastFailClosedExitExact
  /\ QuorumRebroadcastForceFanoutExact
  /\ QuorumRebroadcastVoteReplayExact
  /\ QuorumRebroadcastPayloadDispatchExact
  /\ QuorumRebroadcastBlockSyncDispatchExact
  /\ QuorumRebroadcastMarkingExact

QuorumRebroadcastDispatchCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ QuorumRebroadcastDispatchExactness

Safety ==
  QuorumRebroadcastCoreSafety

=============================================================================
====
