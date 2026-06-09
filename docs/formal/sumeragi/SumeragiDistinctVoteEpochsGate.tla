---- MODULE SumeragiDistinctVoteEpochsGate ----
EXTENDS Integers, FiniteSets

(***************************************************************************
A bounded abstract model for Sumeragi vote-log epoch replay.

This slice pins `distinct_epochs_for_block_votes(...)` and the immediate
payload-ready replay gate in `proposal_handlers.rs`: once a block payload is
available and a commit topology can be resolved, the actor retries Commit-QC
formation once per distinct epoch already present in the vote log for the
exact block hash, height, view, and Commit phase.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

CommitPhase == 1
PreparePhase == 2
TargetHash == 1
OtherHash == 2
TargetHeight == 7
TargetView == 2

Cases == {
  "no_votes",
  "exact_epoch_zero",
  "exact_epoch_five",
  "duplicate_same_epoch",
  "two_epochs",
  "wrong_phase_only",
  "wrong_hash_only",
  "wrong_height_only",
  "wrong_view_only",
  "mixed_filter",
  "key_epoch_mismatch",
  "topology_empty_with_epochs"
}

\* @type: (Int, Int, Int, Int, Int, Int, Int) => <<Int, Int, Int, Int, Int, Int, Int>>;
Vote(id, phase, blockHash, height, view, epoch, keyEpoch) ==
  <<id, phase, blockHash, height, view, epoch, keyEpoch>>

ExactVote(id, epoch) ==
  Vote(id, CommitPhase, TargetHash, TargetHeight, TargetView, epoch, epoch)

\* @type: Str => Set(<<Int, Int, Int, Int, Int, Int, Int>>);
Votes(c) ==
  CASE c = "no_votes" -> {}
    [] c = "exact_epoch_zero" -> {ExactVote(1, 0)}
    [] c = "exact_epoch_five" -> {ExactVote(1, 5)}
    [] c = "duplicate_same_epoch" ->
       {ExactVote(1, 8), ExactVote(2, 8)}
    [] c = "two_epochs" ->
       {ExactVote(1, 1), ExactVote(2, 9)}
    [] c = "wrong_phase_only" ->
       {Vote(1, PreparePhase, TargetHash, TargetHeight, TargetView, 3, 3)}
    [] c = "wrong_hash_only" ->
       {Vote(1, CommitPhase, OtherHash, TargetHeight, TargetView, 4, 4)}
    [] c = "wrong_height_only" ->
       {Vote(1, CommitPhase, TargetHash, TargetHeight + 1, TargetView, 5, 5)}
    [] c = "wrong_view_only" ->
       {Vote(1, CommitPhase, TargetHash, TargetHeight, TargetView + 1, 6, 6)}
    [] c = "mixed_filter" ->
       {ExactVote(1, 1),
        ExactVote(2, 2),
        Vote(3, PreparePhase, TargetHash, TargetHeight, TargetView, 3, 3),
        Vote(4, CommitPhase, OtherHash, TargetHeight, TargetView, 4, 4),
        Vote(5, CommitPhase, TargetHash, TargetHeight + 1, TargetView, 5, 5),
        Vote(6, CommitPhase, TargetHash, TargetHeight, TargetView + 1, 6, 6)}
    [] c = "key_epoch_mismatch" ->
       {Vote(1, CommitPhase, TargetHash, TargetHeight, TargetView, 7, 2)}
    [] c = "topology_empty_with_epochs" ->
       {ExactVote(1, 4), ExactVote(2, 5)}
    [] OTHER -> {}

\* @type: <<Int, Int, Int, Int, Int, Int, Int>> => Bool;
VoteMatches(v) ==
  /\ v[2] = CommitPhase
  /\ v[3] = TargetHash
  /\ v[4] = TargetHeight
  /\ v[5] = TargetView

\* @type: Str => Set(<<Int, Int, Int, Int, Int, Int, Int>>);
MatchingVotes(c) == {v \in Votes(c): VoteMatches(v)}

\* @type: Str => Set(Int);
SpecEpochs(c) == {v[6]: v \in MatchingVotes(c)}

TopologyPresent(c) == c # "topology_empty_with_epochs"

SpecReplayEpochs(c) ==
  IF TopologyPresent(c) THEN SpecEpochs(c) ELSE {}

SpecReplayCount(c) == Cardinality(SpecReplayEpochs(c))

ActualEpochs(c) ==
  CASE Bug = "include_wrong_phase"
       /\ c = "wrong_phase_only" -> {3}
    [] Bug = "include_wrong_hash"
       /\ c = "wrong_hash_only" -> {4}
    [] Bug = "include_wrong_height"
       /\ c = "wrong_height_only" -> {5}
    [] Bug = "include_wrong_view"
       /\ c = "wrong_view_only" -> {6}
    [] Bug = "drop_epoch_zero"
       /\ c = "exact_epoch_zero" -> {}
    [] Bug = "skip_second_epoch"
       /\ c = "two_epochs" -> {1}
    [] Bug = "skip_all_mixed"
       /\ c = "mixed_filter" -> {}
    [] Bug = "use_vote_key_epoch"
       /\ c = "key_epoch_mismatch" ->
       {v[7]: v \in MatchingVotes(c)}
    [] OTHER -> SpecEpochs(c)

ActualReplayEpochs(c) ==
  CASE Bug = "topology_empty_replays"
       /\ c = "topology_empty_with_epochs" -> ActualEpochs(c)
    [] Bug = "topology_present_skips_replay"
       /\ c = "exact_epoch_five" -> {}
    [] OTHER ->
       IF TopologyPresent(c) THEN ActualEpochs(c) ELSE {}

ActualReplayCount(c) ==
  CASE Bug = "duplicate_epoch_replayed_twice"
       /\ c = "duplicate_same_epoch" -> 2
    [] OTHER -> Cardinality(ActualReplayEpochs(c))

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "include_wrong_phase",
       "include_wrong_hash",
       "include_wrong_height",
       "include_wrong_view",
       "drop_epoch_zero",
       "skip_second_epoch",
       "skip_all_mixed",
       "use_vote_key_epoch",
       "topology_empty_replays",
       "topology_present_skips_replay",
       "duplicate_epoch_replayed_twice"
     }
  /\ checked = 0

DistinctVoteEpochsMatchesSpec ==
  /\ \A c \in Cases:
       ActualEpochs(c) = SpecEpochs(c)
  /\ \A c \in Cases:
       ActualReplayEpochs(c) = SpecReplayEpochs(c)
  /\ \A c \in Cases:
       ActualReplayCount(c) = SpecReplayCount(c)

SafetyFast == DistinctVoteEpochsMatchesSpec

BugIncludeWrongPhase ==
  ActualEpochs("wrong_phase_only") = SpecEpochs("wrong_phase_only")

BugIncludeWrongHash ==
  ActualEpochs("wrong_hash_only") = SpecEpochs("wrong_hash_only")

BugIncludeWrongHeight ==
  ActualEpochs("wrong_height_only") = SpecEpochs("wrong_height_only")

BugIncludeWrongView ==
  ActualEpochs("wrong_view_only") = SpecEpochs("wrong_view_only")

BugDropEpochZero ==
  ActualEpochs("exact_epoch_zero") = SpecEpochs("exact_epoch_zero")

BugSkipSecondEpoch ==
  ActualEpochs("two_epochs") = SpecEpochs("two_epochs")

BugSkipAllMixed ==
  ActualEpochs("mixed_filter") = SpecEpochs("mixed_filter")

BugUseVoteKeyEpoch ==
  ActualEpochs("key_epoch_mismatch") = SpecEpochs("key_epoch_mismatch")

BugTopologyEmptyReplays ==
  ActualReplayEpochs("topology_empty_with_epochs") =
    SpecReplayEpochs("topology_empty_with_epochs")

BugTopologyPresentSkipsReplay ==
  ActualReplayEpochs("exact_epoch_five") = SpecReplayEpochs("exact_epoch_five")

BugDuplicateEpochReplayedTwice ==
  ActualReplayCount("duplicate_same_epoch") = SpecReplayCount("duplicate_same_epoch")

====
