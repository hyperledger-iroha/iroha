---- MODULE SumeragiQcRebuildQuorumGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for QC rebuild quorum reachability.

This slice models `qc_rebuild_candidate_may_reach_quorum(...)`, which filters
cached-vote QC rebuild candidates before attempting expensive local QC
formation.  Permissioned candidates are admitted exactly when their signer count
meets the view-specific signature topology commit threshold.  NPoS candidates
are conservative in two directions: when the cached signer set is unavailable
or cannot be mapped into the signature topology, the helper lets later full QC
formation decide; once signer peers are known, empty stake rosters, missing
stake snapshots, failed stake quorum, or stake-quorum errors all fail closed.
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
  "permissioned_under_quorum",
  "permissioned_exact_quorum",
  "permissioned_over_quorum",
  "permissioned_zero_threshold",
  "npos_missing_signer_set",
  "npos_signer_map_error",
  "npos_empty_stake_roster",
  "npos_missing_snapshot",
  "npos_cached_snapshot_quorum",
  "npos_world_snapshot_quorum",
  "npos_stake_quorum_false",
  "npos_stake_quorum_error"
}

NposCases == {
  "npos_missing_signer_set",
  "npos_signer_map_error",
  "npos_empty_stake_roster",
  "npos_missing_snapshot",
  "npos_cached_snapshot_quorum",
  "npos_world_snapshot_quorum",
  "npos_stake_quorum_false",
  "npos_stake_quorum_error"
}

ConsensusMode(c) ==
  IF c \in NposCases THEN "Npos" ELSE "Permissioned"

SignerCount(c) ==
  CASE c = "permissioned_under_quorum" -> 1
    [] c = "permissioned_exact_quorum" -> 2
    [] c = "permissioned_over_quorum" -> 3
    [] c = "permissioned_zero_threshold" -> 0
    [] OTHER -> 0

PermissionedRequired(c) ==
  IF c = "permissioned_zero_threshold" THEN 0 ELSE 2

SignerSetKnown(c) ==
  c # "npos_missing_signer_set"

SignerMapOk(c) ==
  c # "npos_signer_map_error"

StakeRosterNonempty(c) ==
  c # "npos_empty_stake_roster"

StakeSnapshotAvailable(c) ==
  c \in {
    "npos_cached_snapshot_quorum",
    "npos_world_snapshot_quorum",
    "npos_stake_quorum_false",
    "npos_stake_quorum_error"
  }

StakeQuorumResult(c) ==
  CASE c \in {"npos_cached_snapshot_quorum", "npos_world_snapshot_quorum"} -> "true"
    [] c = "npos_stake_quorum_false" -> "false"
    [] c = "npos_stake_quorum_error" -> "error"
    [] OTHER -> "unknown"

SpecMayReach(c) ==
  IF ConsensusMode(c) = "Permissioned"
  THEN SignerCount(c) >= PermissionedRequired(c)
  ELSE
    IF ~SignerSetKnown(c)
    THEN TRUE
    ELSE IF ~SignerMapOk(c)
    THEN TRUE
    ELSE
      /\ StakeRosterNonempty(c)
      /\ StakeSnapshotAvailable(c)
      /\ StakeQuorumResult(c) = "true"

ActualMayReach(c) ==
  CASE Bug = "permissioned_accepts_under_quorum"
       /\ c = "permissioned_under_quorum" -> TRUE
    [] Bug = "permissioned_rejects_exact_quorum"
       /\ c = "permissioned_exact_quorum" -> FALSE
    [] Bug = "permissioned_rejects_over_quorum"
       /\ c = "permissioned_over_quorum" -> FALSE
    [] Bug = "permissioned_rejects_zero_threshold"
       /\ c = "permissioned_zero_threshold" -> FALSE
    [] Bug = "npos_rejects_missing_signer_set"
       /\ c = "npos_missing_signer_set" -> FALSE
    [] Bug = "npos_rejects_signer_map_error"
       /\ c = "npos_signer_map_error" -> FALSE
    [] Bug = "npos_accepts_empty_stake_roster"
       /\ c = "npos_empty_stake_roster" -> TRUE
    [] Bug = "npos_accepts_missing_snapshot"
       /\ c = "npos_missing_snapshot" -> TRUE
    [] Bug = "npos_rejects_cached_snapshot_quorum"
       /\ c = "npos_cached_snapshot_quorum" -> FALSE
    [] Bug = "npos_rejects_world_snapshot_quorum"
       /\ c = "npos_world_snapshot_quorum" -> FALSE
    [] Bug = "npos_accepts_stake_quorum_false"
       /\ c = "npos_stake_quorum_false" -> TRUE
    [] Bug = "npos_accepts_stake_quorum_error"
       /\ c = "npos_stake_quorum_error" -> TRUE
    [] Bug = "npos_requires_known_signers"
       /\ c \in {"npos_missing_signer_set", "npos_signer_map_error"} -> FALSE
    [] Bug = "npos_treats_any_snapshot_as_quorum"
       /\ c = "npos_stake_quorum_false" -> TRUE
    [] OTHER -> SpecMayReach(c)

Matches(c) ==
  ActualMayReach(c) = SpecMayReach(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "permissioned_accepts_under_quorum",
       "permissioned_rejects_exact_quorum",
       "permissioned_rejects_over_quorum",
       "permissioned_rejects_zero_threshold",
       "npos_rejects_missing_signer_set",
       "npos_rejects_signer_map_error",
       "npos_accepts_empty_stake_roster",
       "npos_accepts_missing_snapshot",
       "npos_rejects_cached_snapshot_quorum",
       "npos_rejects_world_snapshot_quorum",
       "npos_accepts_stake_quorum_false",
       "npos_accepts_stake_quorum_error",
       "npos_requires_known_signers",
       "npos_treats_any_snapshot_as_quorum"
     }
  /\ checked = 0

AllCasesMatchSpec ==
  \A c \in Cases: Matches(c)

Safety ==
  AllCasesMatchSpec

PermissionedThreshold ==
  /\ Matches("permissioned_under_quorum")
  /\ Matches("permissioned_exact_quorum")
  /\ Matches("permissioned_over_quorum")

PermissionedZeroThreshold ==
  Matches("permissioned_zero_threshold")

NposUnknownSignerMaterialDefers ==
  /\ Matches("npos_missing_signer_set")
  /\ Matches("npos_signer_map_error")

NposStakeEvidenceRequired ==
  /\ Matches("npos_empty_stake_roster")
  /\ Matches("npos_missing_snapshot")
  /\ Matches("npos_stake_quorum_false")
  /\ Matches("npos_stake_quorum_error")

NposStakeQuorumAdmits ==
  /\ Matches("npos_cached_snapshot_quorum")
  /\ Matches("npos_world_snapshot_quorum")

=============================================================================
====
