---- MODULE SumeragiBlockSyncVotePlaceholderGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for the frontier vote-placeholder side effect in
`handle_block_sync_update(...)`.

Before embedded commit votes are handled, vote-only exact-frontier
BlockSyncUpdates may note frontier vote placeholders when the block is unknown
locally and no missing-block request is active. The placeholder loop records
only embedded commit votes whose phase, hash, height, view, and epoch match the
incoming block; mismatched votes are ignored. Commit-QC and checkpoint sidecars
make the update non-vote-only, while a stake snapshot alone does not. The branch
does not record status, clear requests, defer the update, or return early.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

Cases == {
  "no_votes",
  "valid_vote",
  "two_valid_votes",
  "invalid_phase",
  "invalid_hash",
  "invalid_height",
  "invalid_view",
  "invalid_epoch",
  "mixed_votes",
  "with_qc_sidecar",
  "with_checkpoint_sidecar",
  "with_stake_sidecar",
  "not_exact_frontier",
  "known_local",
  "already_requested"
}

ValidVoteCount(c) ==
  CASE c \in {"valid_vote", "with_stake_sidecar"} -> 1
    [] c = "two_valid_votes" -> 2
    [] c = "mixed_votes" -> 1
    [] OTHER -> 0

InvalidVoteCount(c) ==
  IF c \in {
       "invalid_phase",
       "invalid_hash",
       "invalid_height",
       "invalid_view",
       "invalid_epoch",
       "mixed_votes"
     }
  THEN 1
  ELSE 0

IncomingVoteCount(c) ==
  ValidVoteCount(c) + InvalidVoteCount(c)

HasCommitVotes(c) ==
  IncomingVoteCount(c) > 0

IncomingQc(c) ==
  c = "with_qc_sidecar"

ValidatorCheckpoint(c) ==
  c = "with_checkpoint_sidecar"

StakeSnapshot(c) ==
  c = "with_stake_sidecar"

VoteOnlyFrontierUpdate(c) ==
  /\ HasCommitVotes(c)
  /\ ~IncomingQc(c)
  /\ ~ValidatorCheckpoint(c)

ExactContiguousFrontier(c) ==
  c # "not_exact_frontier"

BlockKnownLocally(c) ==
  c = "known_local"

RequestedMissing(c) ==
  c = "already_requested"

SpecPlaceholderGate(c) ==
  /\ VoteOnlyFrontierUpdate(c)
  /\ ExactContiguousFrontier(c)
  /\ ~BlockKnownLocally(c)
  /\ ~RequestedMissing(c)

SpecPlaceholderCount(c) ==
  IF SpecPlaceholderGate(c) THEN ValidVoteCount(c) ELSE 0

SpecInvalidVotesIgnored(c) ==
  TRUE

SpecUsesVoteSubject(c) ==
  TRUE

SpecPayloadMarker(c) ==
  IF SpecPlaceholderCount(c) > 0 THEN "none" ELSE "not_called"

SpecRecordsStatus(c) ==
  FALSE

SpecClearsMissing(c) ==
  FALSE

SpecDefers(c) ==
  FALSE

SpecReturnKind(c) ==
  "continue"

ActualPlaceholderGate(c) ==
  CASE Bug = "gate_ignores_exact_frontier"
       /\ c = "not_exact_frontier" -> TRUE
    [] Bug = "gate_ignores_known_local"
       /\ c = "known_local" -> TRUE
    [] Bug = "gate_ignores_requested"
       /\ c = "already_requested" -> TRUE
    [] Bug = "gate_allows_qc_sidecar"
       /\ c = "with_qc_sidecar" -> TRUE
    [] Bug = "gate_allows_checkpoint_sidecar"
       /\ c = "with_checkpoint_sidecar" -> TRUE
    [] Bug = "gate_blocks_stake_sidecar"
       /\ c = "with_stake_sidecar" -> FALSE
    [] OTHER -> SpecPlaceholderGate(c)

ActualPlaceholderCount(c) ==
  IF ~ActualPlaceholderGate(c) THEN 0
  ELSE CASE Bug = "valid_vote_not_noted"
            /\ c = "valid_vote" -> 0
         [] Bug = "second_valid_vote_not_noted"
            /\ c = "two_valid_votes" -> 1
         [] Bug = "invalid_phase_noted"
            /\ c = "invalid_phase" -> 1
         [] Bug = "invalid_hash_noted"
            /\ c = "invalid_hash" -> 1
         [] Bug = "invalid_height_noted"
            /\ c = "invalid_height" -> 1
         [] Bug = "invalid_view_noted"
            /\ c = "invalid_view" -> 1
         [] Bug = "invalid_epoch_noted"
            /\ c = "invalid_epoch" -> 1
         [] Bug = "mixed_invalid_noted"
            /\ c = "mixed_votes" -> 2
         [] OTHER -> ValidVoteCount(c)

ActualInvalidVotesIgnored(c) ==
  IF c \in {
       "invalid_phase",
       "invalid_hash",
       "invalid_height",
       "invalid_view",
       "invalid_epoch",
       "mixed_votes"
     }
  THEN ActualPlaceholderCount(c) = ValidVoteCount(c)
  ELSE TRUE

ActualUsesVoteSubject(c) ==
  IF ActualPlaceholderCount(c) = 0 THEN TRUE
  ELSE ~(Bug = "placeholder_uses_block_subject" /\ c = "valid_vote")

ActualPayloadMarker(c) ==
  IF ActualPlaceholderCount(c) = 0 THEN "not_called"
  ELSE CASE Bug = "placeholder_payload_some"
            /\ c = "valid_vote" -> "some"
         [] OTHER -> "none"

ActualRecordsStatus(c) ==
  Bug = "placeholder_records_status" /\ c = "valid_vote"

ActualClearsMissing(c) ==
  Bug = "placeholder_clears_missing" /\ c = "valid_vote"

ActualDefers(c) ==
  Bug = "placeholder_defers_update" /\ c = "valid_vote"

ActualReturnKind(c) ==
  CASE Bug = "placeholder_returns_early"
       /\ c = "valid_vote" -> "Ok"
    [] OTHER -> "continue"

Matches(c) ==
  /\ ActualPlaceholderGate(c) = SpecPlaceholderGate(c)
  /\ ActualPlaceholderCount(c) = SpecPlaceholderCount(c)
  /\ ActualInvalidVotesIgnored(c) = SpecInvalidVotesIgnored(c)
  /\ ActualUsesVoteSubject(c) = SpecUsesVoteSubject(c)
  /\ ActualPayloadMarker(c) = SpecPayloadMarker(c)
  /\ ActualRecordsStatus(c) = SpecRecordsStatus(c)
  /\ ActualClearsMissing(c) = SpecClearsMissing(c)
  /\ ActualDefers(c) = SpecDefers(c)
  /\ ActualReturnKind(c) = SpecReturnKind(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "valid_vote_not_noted",
       "second_valid_vote_not_noted",
       "invalid_phase_noted",
       "invalid_hash_noted",
       "invalid_height_noted",
       "invalid_view_noted",
       "invalid_epoch_noted",
       "mixed_invalid_noted",
       "gate_ignores_exact_frontier",
       "gate_ignores_known_local",
       "gate_ignores_requested",
       "gate_allows_qc_sidecar",
       "gate_allows_checkpoint_sidecar",
       "gate_blocks_stake_sidecar",
       "placeholder_uses_block_subject",
       "placeholder_payload_some",
       "placeholder_records_status",
       "placeholder_clears_missing",
       "placeholder_defers_update",
       "placeholder_returns_early"
     }
  /\ checked = 0

SafetyFast ==
  \A c \in Cases: Matches(c)

NoVotesNoPlaceholder ==
  Matches("no_votes")

ValidVoteNoted ==
  Matches("valid_vote")

TwoValidVotesNoted ==
  Matches("two_valid_votes")

InvalidPhaseIgnored ==
  Matches("invalid_phase")

InvalidHashIgnored ==
  Matches("invalid_hash")

InvalidHeightIgnored ==
  Matches("invalid_height")

InvalidViewIgnored ==
  Matches("invalid_view")

InvalidEpochIgnored ==
  Matches("invalid_epoch")

MixedVotesFilterInvalid ==
  Matches("mixed_votes")

QcSidecarBlocksPlaceholder ==
  Matches("with_qc_sidecar")

CheckpointSidecarBlocksPlaceholder ==
  Matches("with_checkpoint_sidecar")

StakeSidecarStillAllowsPlaceholder ==
  Matches("with_stake_sidecar")

ExactFrontierRequired ==
  Matches("not_exact_frontier")

KnownLocalBlocksPlaceholder ==
  Matches("known_local")

RequestedBlocksPlaceholder ==
  Matches("already_requested")

PlaceholderUsesVoteSubject ==
  Matches("valid_vote")

PlaceholderPayloadNone ==
  Matches("valid_vote")

PlaceholderHasNoStatus ==
  Matches("valid_vote")

PlaceholderDoesNotClear ==
  Matches("valid_vote")

PlaceholderDoesNotDefer ==
  Matches("valid_vote")

PlaceholderContinues ==
  Matches("valid_vote")

=============================================================================
====
