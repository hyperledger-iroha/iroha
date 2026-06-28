---- MODULE SumeragiApplyCachedQcsGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for cached BlockSyncUpdate proof attachment.

This slice captures `Actor::apply_cached_qcs_to_block_sync_update(...)`. It
models the externally relevant attachment policy: an existing commit QC is
preserved; otherwise topology-backed cache/history evidence wins over raw
cache evidence, signer-history derivation, and world fallback. A missing
validator checkpoint is synthesized from the final commit QC, existing
checkpoints are preserved, NPoS stake snapshots are repaired when absent or
mismatched, and cached commit votes fill only an empty vote list.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

ExistingQCPreserved == "existing_qc_preserved"
CheckpointTopologyHistory == "checkpoint_topology_history"
StateTopologyCache == "state_topology_cache"
RawCacheFallback == "raw_cache_fallback"
SignerHistoryDerivedPermissioned == "signer_history_derived_permissioned"
SignerHistoryDerivedNposRecordStake == "signer_history_derived_npos_record_stake"
WorldFallback == "world_fallback"
NoQcNoVotes == "no_qc_no_votes"
CheckpointFromQc == "checkpoint_from_qc"
ExistingCheckpointPreserved == "existing_checkpoint_preserved"
NposStakePreserveMatching == "npos_stake_preserve_matching"
NposStakeRepairMissing == "npos_stake_repair_missing"
NposStakeRepairMismatch == "npos_stake_repair_mismatch"
PermissionedNoStakeRepair == "permissioned_no_stake_repair"
VotesAttachedWhenEmpty == "votes_attached_when_empty"
VotesPreservedWhenExisting == "votes_preserved_when_existing"
VotesWrongContextIgnored == "votes_wrong_context_ignored"

Cases == {
  ExistingQCPreserved,
  CheckpointTopologyHistory,
  StateTopologyCache,
  RawCacheFallback,
  SignerHistoryDerivedPermissioned,
  SignerHistoryDerivedNposRecordStake,
  WorldFallback,
  NoQcNoVotes,
  CheckpointFromQc,
  ExistingCheckpointPreserved,
  NposStakePreserveMatching,
  NposStakeRepairMissing,
  NposStakeRepairMismatch,
  PermissionedNoStakeRepair,
  VotesAttachedWhenEmpty,
  VotesPreservedWhenExisting,
  VotesWrongContextIgnored
}

NoneVal == "none"
ExistingQc == "existing_qc"
TopologyQc == "topology_qc"
RawCacheQc == "raw_cache_qc"
SignerQc == "signer_qc"
WorldQc == "world_qc"

QcValues == {NoneVal, ExistingQc, TopologyQc, RawCacheQc, SignerQc, WorldQc}

ExistingCheckpoint == "existing_checkpoint"
CheckpointFromFinalQc == "checkpoint_from_final_qc"

CheckpointValues == {NoneVal, ExistingCheckpoint, CheckpointFromFinalQc}

ExistingStake == "existing_stake"
RecordStake == "record_stake"
RepairedStake == "repaired_stake"
BadStake == "bad_stake"

StakeValues == {NoneVal, ExistingStake, RecordStake, RepairedStake, BadStake}

ExistingVotes == "existing_votes"
MatchingVotes == "matching_votes"

VoteValues == {NoneVal, ExistingVotes, MatchingVotes}

HasInitialCheckpoint(c) ==
  c \in {CheckpointTopologyHistory, ExistingCheckpointPreserved}

SpecFinalQc(c) ==
  CASE c = ExistingQCPreserved -> ExistingQc
    [] c \in {CheckpointTopologyHistory, StateTopologyCache} -> TopologyQc
    [] c \in {RawCacheFallback, CheckpointFromQc, ExistingCheckpointPreserved,
               NposStakePreserveMatching, NposStakeRepairMissing,
               NposStakeRepairMismatch, PermissionedNoStakeRepair} ->
      RawCacheQc
    [] c \in {SignerHistoryDerivedPermissioned,
              SignerHistoryDerivedNposRecordStake} ->
      SignerQc
    [] c = WorldFallback -> WorldQc
    [] OTHER -> NoneVal

SpecFinalCheckpoint(c) ==
  IF HasInitialCheckpoint(c) THEN ExistingCheckpoint
  ELSE IF SpecFinalQc(c) # NoneVal THEN CheckpointFromFinalQc
  ELSE NoneVal

SpecFinalStake(c) ==
  CASE c = SignerHistoryDerivedNposRecordStake -> RecordStake
    [] c = NposStakePreserveMatching -> ExistingStake
    [] c \in {NposStakeRepairMissing, NposStakeRepairMismatch} -> RepairedStake
    [] c = PermissionedNoStakeRepair -> BadStake
    [] OTHER -> NoneVal

SpecFinalVotes(c) ==
  CASE c = VotesPreservedWhenExisting -> ExistingVotes
    [] c = VotesAttachedWhenEmpty -> MatchingVotes
    [] OTHER -> NoneVal

ActualFinalQc(c) ==
  CASE Bug = "overwrite_existing_qc"
       /\ c = ExistingQCPreserved ->
      RawCacheQc
    [] Bug = "skip_topology_qc"
       /\ c = CheckpointTopologyHistory ->
      NoneVal
    [] Bug = "raw_cache_before_topology"
       /\ c = StateTopologyCache ->
      RawCacheQc
    [] Bug = "skip_raw_cache"
       /\ c = RawCacheFallback ->
      NoneVal
    [] Bug = "skip_signer_history"
       /\ c \in {SignerHistoryDerivedPermissioned,
                 SignerHistoryDerivedNposRecordStake} ->
      NoneVal
    [] Bug = "skip_world_fallback"
       /\ c = WorldFallback ->
      NoneVal
    [] OTHER -> SpecFinalQc(c)

ActualFinalCheckpoint(c) ==
  CASE Bug = "create_checkpoint_without_qc"
       /\ c = NoQcNoVotes ->
      CheckpointFromFinalQc
    [] Bug = "skip_checkpoint_from_qc"
       /\ c = CheckpointFromQc ->
      NoneVal
    [] Bug = "overwrite_existing_checkpoint"
       /\ c = ExistingCheckpointPreserved ->
      CheckpointFromFinalQc
    [] OTHER -> SpecFinalCheckpoint(c)

ActualFinalStake(c) ==
  CASE Bug = "skip_npos_stake_repair_missing"
       /\ c = NposStakeRepairMissing ->
      NoneVal
    [] Bug = "preserve_mismatched_npos_stake"
       /\ c = NposStakeRepairMismatch ->
      BadStake
    [] Bug = "repair_permissioned_stake"
       /\ c = PermissionedNoStakeRepair ->
      RepairedStake
    [] Bug = "skip_record_stake_clone"
       /\ c = SignerHistoryDerivedNposRecordStake ->
      NoneVal
    [] OTHER -> SpecFinalStake(c)

ActualFinalVotes(c) ==
  CASE Bug = "overwrite_existing_votes"
       /\ c = VotesPreservedWhenExisting ->
      MatchingVotes
    [] Bug = "skip_vote_log"
       /\ c = VotesAttachedWhenEmpty ->
      NoneVal
    [] Bug = "accept_wrong_context_votes"
       /\ c = VotesWrongContextIgnored ->
      MatchingVotes
    [] OTHER -> SpecFinalVotes(c)

Bugs == {
  "none",
  "overwrite_existing_qc",
  "skip_topology_qc",
  "raw_cache_before_topology",
  "skip_raw_cache",
  "skip_signer_history",
  "skip_world_fallback",
  "create_checkpoint_without_qc",
  "skip_checkpoint_from_qc",
  "overwrite_existing_checkpoint",
  "skip_npos_stake_repair_missing",
  "preserve_mismatched_npos_stake",
  "repair_permissioned_stake",
  "skip_record_stake_clone",
  "overwrite_existing_votes",
  "skip_vote_log",
  "accept_wrong_context_votes"
}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..1
  /\ \A c \in Cases:
       /\ SpecFinalQc(c) \in QcValues
       /\ ActualFinalQc(c) \in QcValues
       /\ SpecFinalCheckpoint(c) \in CheckpointValues
       /\ ActualFinalCheckpoint(c) \in CheckpointValues
       /\ SpecFinalStake(c) \in StakeValues
       /\ ActualFinalStake(c) \in StakeValues
       /\ SpecFinalVotes(c) \in VoteValues
       /\ ActualFinalVotes(c) \in VoteValues

CommitQcSelectionMatchesSpec ==
  \A c \in Cases:
    ActualFinalQc(c) = SpecFinalQc(c)

CheckpointAttachmentMatchesSpec ==
  \A c \in Cases:
    ActualFinalCheckpoint(c) = SpecFinalCheckpoint(c)

StakeSnapshotAttachmentMatchesSpec ==
  \A c \in Cases:
    ActualFinalStake(c) = SpecFinalStake(c)

CommitVotesAttachmentMatchesSpec ==
  \A c \in Cases:
    ActualFinalVotes(c) = SpecFinalVotes(c)

NoSpuriousCheckpointWithoutQc ==
  \A c \in Cases:
    SpecFinalQc(c) = NoneVal => ActualFinalCheckpoint(c) # CheckpointFromFinalQc

NoBugInvariant ==
  /\ CommitQcSelectionMatchesSpec
  /\ CheckpointAttachmentMatchesSpec
  /\ StakeSnapshotAttachmentMatchesSpec
  /\ CommitVotesAttachmentMatchesSpec
  /\ NoSpuriousCheckpointWithoutQc

ApplyCachedQcsExactness ==
  /\ CommitQcSelectionMatchesSpec
  /\ CheckpointAttachmentMatchesSpec
  /\ StakeSnapshotAttachmentMatchesSpec
  /\ CommitVotesAttachmentMatchesSpec
  /\ NoSpuriousCheckpointWithoutQc

ApplyCachedQcsCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ ApplyCachedQcsExactness

SafetyFast ==
  ApplyCachedQcsExactness

BugOverwriteExistingQc == NoBugInvariant
BugSkipTopologyQc == NoBugInvariant
BugRawCacheBeforeTopology == NoBugInvariant
BugSkipRawCache == NoBugInvariant
BugSkipSignerHistory == NoBugInvariant
BugSkipWorldFallback == NoBugInvariant
BugCreateCheckpointWithoutQc == NoBugInvariant
BugSkipCheckpointFromQc == NoBugInvariant
BugOverwriteExistingCheckpoint == NoBugInvariant
BugSkipNposStakeRepairMissing == NoBugInvariant
BugPreserveMismatchedNposStake == NoBugInvariant
BugRepairPermissionedStake == NoBugInvariant
BugSkipRecordStakeClone == NoBugInvariant
BugOverwriteExistingVotes == NoBugInvariant
BugSkipVoteLog == NoBugInvariant
BugAcceptWrongContextVotes == NoBugInvariant

====
