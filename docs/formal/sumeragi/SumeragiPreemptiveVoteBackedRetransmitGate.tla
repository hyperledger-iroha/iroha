---- MODULE SumeragiPreemptiveVoteBackedRetransmitGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for the pre-timeout vote-backed frontier retransmit
handoff around `preemptive_rebroadcast_vote_backed_frontier_block(...)`.

The resend-window arithmetic and downstream rebroadcast dispatch are covered by
adjacent models. This slice pins the handoff contract between them:
- only admitted pre-timeout candidates are processed,
- absent pending blocks cannot produce work or synthesize state,
- vote-roster targets are preferred and empty vote rosters fall back to commit
  topology,
- empty combined targets fail closed while preserving pending state,
- returned progress requires an emitted vote, BlockSyncUpdate, or BlockCreated
  replay, and
- the downstream near-quorum flag is exactly `vote_count < min_votes_for_commit`.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

NoSource == 0
VoteRoster == 1
CommitTopology == 2

NoWindow == "NoWindow"
MissingVotes == "MissingVotes"
HasQc == "HasQc"
ValidationInflight == "ValidationInflight"
MissingLocalData == "MissingLocalData"
RecoveryBlocked == "RecoveryBlocked"
ProgressBeforeWindow == "ProgressBeforeWindow"
ProgressAtTimeout == "ProgressAtTimeout"
NotDue == "NotDue"
NoPending == "NoPending"
NoTargets == "NoTargets"
VoteRosterVotes == "VoteRosterVotes"
CommitFallbackBlockSync == "CommitFallbackBlockSync"
NoOutput == "NoOutput"
VotesOnly == "VotesOnly"
BlockSyncOnly == "BlockSyncOnly"
BlockOnly == "BlockOnly"
MultiOutput == "MultiOutput"
AtQuorumOutput == "AtQuorumOutput"

Cases == {
  NoWindow,
  MissingVotes,
  HasQc,
  ValidationInflight,
  MissingLocalData,
  RecoveryBlocked,
  ProgressBeforeWindow,
  ProgressAtTimeout,
  NotDue,
  NoPending,
  NoTargets,
  VoteRosterVotes,
  CommitFallbackBlockSync,
  NoOutput,
  VotesOnly,
  BlockSyncOnly,
  BlockOnly,
  MultiOutput,
  AtQuorumOutput
}

CandidateRejectCases == {
  NoWindow,
  MissingVotes,
  HasQc,
  ValidationInflight,
  MissingLocalData,
  RecoveryBlocked,
  ProgressBeforeWindow,
  ProgressAtTimeout,
  NotDue
}

CandidateAcceptCases == {
  NoPending,
  NoTargets,
  VoteRosterVotes,
  CommitFallbackBlockSync,
  NoOutput,
  VotesOnly,
  BlockSyncOnly,
  BlockOnly,
  MultiOutput,
  AtQuorumOutput
}

TargetSelectionCases == {
  VoteRosterVotes,
  CommitFallbackBlockSync,
  NoTargets
}

ActionOutputCases == {
  NoOutput,
  VotesOnly,
  BlockSyncOnly,
  BlockOnly,
  MultiOutput,
  AtQuorumOutput
}

PendingRetentionCases == {
  NoPending,
  NoTargets,
  MultiOutput
}

NearFlagCases == {
  VoteRosterVotes,
  AtQuorumOutput
}

Bugs == {
  "none",
  "no_window_allows",
  "missing_votes_allows",
  "qc_allows",
  "validation_allows",
  "missing_data_allows",
  "recovery_block_allows",
  "early_stall_allows",
  "timeout_boundary_allows",
  "due_ignored",
  "no_pending_actions",
  "no_pending_creates_pending",
  "vote_roster_ignored",
  "commit_fallback_skipped",
  "no_targets_rebroadcasts",
  "no_targets_drops_pending",
  "votes_not_action",
  "block_sync_not_action",
  "block_not_action",
  "outputless_action",
  "action_drops_pending",
  "near_flag_always_false",
  "near_flag_always_true"
}

PendingBefore(c) == c # NoPending
ResendWindowAvailable(c) == c # NoWindow
HasVotes(c) == c # MissingVotes
HasQcPresent(c) == c = HasQc
ValidationInFlight(c) == c = ValidationInflight
MissingLocalPayload(c) == c = MissingLocalData
AllowedUnderRecovery(c) == c # RecoveryBlocked
ProgressStallAge(c) ==
  CASE c = ProgressBeforeWindow -> 4
    [] c = ProgressAtTimeout -> 10
    [] OTHER -> 5
ResendWindow(c) == 5
QuorumTimeout(c) == 10
Due(c) == c # NotDue
VoteTargets(c) == c # CommitFallbackBlockSync /\ c # NoTargets
CommitTargets(c) == c # NoTargets
DownstreamVotes(c) ==
  c \notin {CommitFallbackBlockSync, NoOutput, BlockSyncOnly, BlockOnly, AtQuorumOutput}
DownstreamBlockSync(c) == c \in {CommitFallbackBlockSync, BlockSyncOnly, MultiOutput}
DownstreamBlock(c) == c \in {BlockOnly, MultiOutput, AtQuorumOutput}
VoteCount(c) ==
  CASE c = MissingVotes -> 0
    [] c = AtQuorumOutput -> 3
    [] OTHER -> 1
MinVotes(c) == 3

NearQuorumFlag(c) == VoteCount(c) < MinVotes(c)
AnyDownstreamOutput(c) ==
  DownstreamVotes(c) \/ DownstreamBlockSync(c) \/ DownstreamBlock(c)

SpecCandidate(c) ==
  /\ ResendWindowAvailable(c)
  /\ HasVotes(c)
  /\ ~HasQcPresent(c)
  /\ ~ValidationInFlight(c)
  /\ ~MissingLocalPayload(c)
  /\ AllowedUnderRecovery(c)
  /\ ProgressStallAge(c) >= ResendWindow(c)
  /\ ProgressStallAge(c) < QuorumTimeout(c)
  /\ Due(c)

SpecSelectedSource(c) ==
  IF ~SpecCandidate(c) \/ ~PendingBefore(c) THEN NoSource
  ELSE IF VoteTargets(c) THEN VoteRoster
  ELSE IF CommitTargets(c) THEN CommitTopology
  ELSE NoSource

SpecRebroadcast(c) == SpecSelectedSource(c) # NoSource
SpecAction(c) == SpecRebroadcast(c) /\ AnyDownstreamOutput(c)
SpecPendingAfter(c) == PendingBefore(c)
SpecNearFlagOk(c) == TRUE

ActualCandidate(c) ==
  \/ SpecCandidate(c)
  \/ Bug = "no_window_allows" /\ c = NoWindow
  \/ Bug = "missing_votes_allows" /\ c = MissingVotes
  \/ Bug = "qc_allows" /\ c = HasQc
  \/ Bug = "validation_allows" /\ c = ValidationInflight
  \/ Bug = "missing_data_allows" /\ c = MissingLocalData
  \/ Bug = "recovery_block_allows" /\ c = RecoveryBlocked
  \/ Bug = "early_stall_allows" /\ c = ProgressBeforeWindow
  \/ Bug = "timeout_boundary_allows" /\ c = ProgressAtTimeout
  \/ Bug = "due_ignored" /\ c = NotDue

ActualSelectedSource(c) ==
  IF ~ActualCandidate(c) THEN NoSource
  ELSE IF ~PendingBefore(c) /\ Bug # "no_pending_actions" THEN NoSource
  ELSE IF Bug = "vote_roster_ignored" /\ c = VoteRosterVotes THEN CommitTopology
  ELSE IF Bug = "commit_fallback_skipped" /\ c = CommitFallbackBlockSync THEN NoSource
  ELSE IF Bug = "no_targets_rebroadcasts" /\ c = NoTargets THEN VoteRoster
  ELSE IF VoteTargets(c) THEN VoteRoster
  ELSE IF CommitTargets(c) THEN CommitTopology
  ELSE NoSource

ActualRebroadcast(c) == ActualSelectedSource(c) # NoSource

ActualAnyDownstreamOutput(c) ==
  IF Bug = "votes_not_action" /\ c = VotesOnly THEN
    DownstreamBlockSync(c) \/ DownstreamBlock(c)
  ELSE IF Bug = "block_sync_not_action" /\ c = BlockSyncOnly THEN
    DownstreamVotes(c) \/ DownstreamBlock(c)
  ELSE IF Bug = "block_not_action" /\ c = BlockOnly THEN
    DownstreamVotes(c) \/ DownstreamBlockSync(c)
  ELSE IF Bug = "outputless_action" /\ c = NoOutput THEN
    TRUE
  ELSE
    AnyDownstreamOutput(c)

ActualAction(c) == ActualRebroadcast(c) /\ ActualAnyDownstreamOutput(c)

ActualPendingAfter(c) ==
  IF Bug = "no_pending_creates_pending" /\ c = NoPending THEN TRUE
  ELSE IF Bug = "no_targets_drops_pending" /\ c = NoTargets THEN FALSE
  ELSE IF Bug = "action_drops_pending" /\ c = MultiOutput THEN FALSE
  ELSE PendingBefore(c)

ActualNearFlagOk(c) ==
  IF ~ActualRebroadcast(c) THEN TRUE
  ELSE IF Bug = "near_flag_always_false" /\ c = VoteRosterVotes THEN
    FALSE = NearQuorumFlag(c)
  ELSE IF Bug = "near_flag_always_true" /\ c = AtQuorumOutput THEN
    TRUE = NearQuorumFlag(c)
  ELSE
    TRUE

CInit == Bug \in Bugs

Init == checked = 0

Next == UNCHANGED vars

TypeInvariant ==
  /\ checked = 0
  /\ Bug \in Bugs

CandidateStable ==
  \A c \in Cases: ActualCandidate(c) = SpecCandidate(c)

SelectionStable ==
  \A c \in Cases: ActualSelectedSource(c) = SpecSelectedSource(c)

DispatchStable ==
  \A c \in Cases: ActualRebroadcast(c) = SpecRebroadcast(c)

ActionStable ==
  \A c \in Cases: ActualAction(c) = SpecAction(c)

PendingStable ==
  \A c \in Cases: ActualPendingAfter(c) = SpecPendingAfter(c)

NearFlagStable ==
  \A c \in Cases: ActualNearFlagOk(c) = SpecNearFlagOk(c)

PreemptiveCandidateRejectExact ==
  \A c \in CandidateRejectCases:
    /\ ~SpecCandidate(c)
    /\ ActualCandidate(c) = FALSE
    /\ ActualSelectedSource(c) = NoSource
    /\ ActualRebroadcast(c) = FALSE
    /\ ActualAction(c) = FALSE
    /\ ActualPendingAfter(c) = SpecPendingAfter(c)

PreemptiveCandidateAcceptExact ==
  \A c \in CandidateAcceptCases:
    /\ SpecCandidate(c)
    /\ ActualCandidate(c) = TRUE
    /\ ActualSelectedSource(c) = SpecSelectedSource(c)
    /\ ActualRebroadcast(c) = SpecRebroadcast(c)

PreemptiveMissingPendingExact ==
  /\ SpecCandidate(NoPending)
  /\ ~PendingBefore(NoPending)
  /\ ActualSelectedSource(NoPending) = NoSource
  /\ ActualRebroadcast(NoPending) = FALSE
  /\ ActualAction(NoPending) = FALSE
  /\ ActualPendingAfter(NoPending) = FALSE

PreemptiveTargetSelectionExact ==
  /\ \A c \in TargetSelectionCases:
       /\ ActualSelectedSource(c) = SpecSelectedSource(c)
       /\ ActualRebroadcast(c) = SpecRebroadcast(c)
       /\ ActualAction(c) = SpecAction(c)
  /\ ActualSelectedSource(VoteRosterVotes) = VoteRoster
  /\ ActualSelectedSource(CommitFallbackBlockSync) = CommitTopology
  /\ ActualSelectedSource(NoTargets) = NoSource

PreemptiveActionOutputExact ==
  \A c \in ActionOutputCases:
    /\ ActualAnyDownstreamOutput(c) = AnyDownstreamOutput(c)
    /\ ActualAction(c) = SpecAction(c)
    /\ ActualPendingAfter(c) = SpecPendingAfter(c)

PreemptivePendingRetentionExact ==
  \A c \in PendingRetentionCases:
    /\ ActualPendingAfter(c) = SpecPendingAfter(c)
    /\ (ActualAction(c) => ActualPendingAfter(c))

PreemptiveNearQuorumFlagExact ==
  /\ \A c \in NearFlagCases:
       /\ ActualRebroadcast(c)
       /\ ActualNearFlagOk(c) = SpecNearFlagOk(c)
  /\ NearQuorumFlag(VoteRosterVotes) = TRUE
  /\ NearQuorumFlag(AtQuorumOutput) = FALSE

PreemptiveVoteBackedRetransmitCoreSafety ==
  /\ CandidateStable
  /\ SelectionStable
  /\ DispatchStable
  /\ ActionStable
  /\ PendingStable
  /\ NearFlagStable

SafetyFast == PreemptiveVoteBackedRetransmitCoreSafety

PreemptiveVoteBackedRetransmitExactness ==
  /\ PreemptiveVoteBackedRetransmitCoreSafety
  /\ PreemptiveCandidateRejectExact
  /\ PreemptiveCandidateAcceptExact
  /\ PreemptiveMissingPendingExact
  /\ PreemptiveTargetSelectionExact
  /\ PreemptiveActionOutputExact
  /\ PreemptivePendingRetentionExact
  /\ PreemptiveNearQuorumFlagExact

====
