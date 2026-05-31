---- MODULE SumeragiProposalStaleVoteGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for proposal-side stale same-height vote gates.

This slice pins `local_same_height_vote_blocks_fresh_proposal(...)`,
`local_same_height_vote_blocks_fresh_proposal_assembly(...)`, and
`stale_local_commit_vote_allows_proposal_assembly_after_missing_qc_repair(...)`.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

BaseCases == {
  "future_view_blocks",
  "commit_non_parent_blocks",
  "same_view_hard_lock",
  "same_view_live_material",
  "same_view_clear",
  "resilience_disabled_old_vote",
  "old_vote_hard_lock",
  "old_vote_fresh_pending",
  "old_vote_active_owner_mid_age",
  "old_vote_pending_stale",
  "old_vote_recovery_age_exhausted",
  "old_vote_view_gap_exhausted",
  "frontier_commit_qc_blocks",
  "frontier_competing_lock_blocks",
  "frontier_no_progress"
}

RepairCases == {
  "repair_parent_marker",
  "repair_retired_pending",
  "repair_retry_aborted_pending",
  "repair_invalid_pending",
  "repair_stale_no_qc_pending",
  "repair_absent_pending",
  "repair_fresh_pending",
  "repair_commit_qc_pending",
  "repair_resilience_disabled",
  "repair_prepare_phase",
  "repair_equal_view",
  "repair_nonfrontier_height",
  "repair_missing_liveness",
  "repair_highest_not_parent",
  "repair_highest_not_canonical",
  "repair_consensus_lock",
  "repair_observed_qc",
  "repair_any_recoverable_qc",
  "repair_vote_lock",
  "repair_inflight_valid"
}

AssemblyCases == {
  "assembly_base_allows",
  "assembly_base_blocks_no_repair",
  "assembly_repair_parent",
  "assembly_repair_retired",
  "assembly_repair_fresh_pending"
}

BoolToInt(b) == IF b THEN 1 ELSE 0

SpecBaseBlocks(c) ==
  c \in {
    "future_view_blocks",
    "commit_non_parent_blocks",
    "same_view_hard_lock",
    "same_view_live_material",
    "resilience_disabled_old_vote",
    "old_vote_hard_lock",
    "old_vote_fresh_pending",
    "old_vote_active_owner_mid_age",
    "frontier_commit_qc_blocks",
    "frontier_competing_lock_blocks"
  }

ActualBaseBlocks(c) ==
  CASE Bug = "base_allows_future_view"
       /\ c = "future_view_blocks" -> FALSE
    [] Bug = "base_allows_commit_conflict"
       /\ c = "commit_non_parent_blocks" -> FALSE
    [] Bug = "base_ignores_same_view_hard_lock"
       /\ c = "same_view_hard_lock" -> FALSE
    [] Bug = "base_ignores_same_view_live_material"
       /\ c = "same_view_live_material" -> FALSE
    [] Bug = "base_blocks_same_view_clear"
       /\ c = "same_view_clear" -> TRUE
    [] Bug = "base_resilience_disabled_allows_old_vote"
       /\ c = "resilience_disabled_old_vote" -> FALSE
    [] Bug = "base_ignores_old_vote_hard_lock"
       /\ c = "old_vote_hard_lock" -> FALSE
    [] Bug = "base_ignores_fresh_pending"
       /\ c = "old_vote_fresh_pending" -> FALSE
    [] Bug = "base_active_owner_uses_short_window"
       /\ c = "old_vote_active_owner_mid_age" -> FALSE
    [] Bug = "base_stale_pending_still_blocks"
       /\ c = "old_vote_pending_stale" -> TRUE
    [] Bug = "base_recovery_age_exhaustion_ignored"
       /\ c = "old_vote_recovery_age_exhausted" -> TRUE
    [] Bug = "base_view_gap_exhaustion_ignored"
       /\ c = "old_vote_view_gap_exhausted" -> TRUE
    [] Bug = "base_ignores_frontier_commit_qc"
       /\ c = "frontier_commit_qc_blocks" -> FALSE
    [] Bug = "base_ignores_frontier_competing_lock"
       /\ c = "frontier_competing_lock_blocks" -> FALSE
    [] Bug = "base_frontier_no_progress_blocks"
       /\ c = "frontier_no_progress" -> TRUE
    [] OTHER -> SpecBaseBlocks(c)

SpecBaseOutput(c) ==
  BoolToInt(SpecBaseBlocks(c))

ActualBaseOutput(c) ==
  BoolToInt(ActualBaseBlocks(c))

SpecRepairAllows(c) ==
  c \in {
    "repair_parent_marker",
    "repair_retired_pending",
    "repair_retry_aborted_pending",
    "repair_invalid_pending",
    "repair_stale_no_qc_pending",
    "repair_absent_pending"
  }

ActualRepairAllows(c) ==
  CASE Bug = "repair_rejects_parent_marker"
       /\ c = "repair_parent_marker" -> FALSE
    [] Bug = "repair_rejects_retired_pending"
       /\ c = "repair_retired_pending" -> FALSE
    [] Bug = "repair_rejects_retry_aborted_pending"
       /\ c = "repair_retry_aborted_pending" -> FALSE
    [] Bug = "repair_rejects_invalid_pending"
       /\ c = "repair_invalid_pending" -> FALSE
    [] Bug = "repair_rejects_stale_no_qc_pending"
       /\ c = "repair_stale_no_qc_pending" -> FALSE
    [] Bug = "repair_rejects_absent_pending"
       /\ c = "repair_absent_pending" -> FALSE
    [] Bug = "repair_allows_fresh_pending"
       /\ c = "repair_fresh_pending" -> TRUE
    [] Bug = "repair_allows_commit_qc_pending"
       /\ c = "repair_commit_qc_pending" -> TRUE
    [] Bug = "repair_ignores_resilience"
       /\ c = "repair_resilience_disabled" -> TRUE
    [] Bug = "repair_allows_prepare_phase"
       /\ c = "repair_prepare_phase" -> TRUE
    [] Bug = "repair_allows_equal_view"
       /\ c = "repair_equal_view" -> TRUE
    [] Bug = "repair_allows_nonfrontier_height"
       /\ c = "repair_nonfrontier_height" -> TRUE
    [] Bug = "repair_allows_missing_liveness"
       /\ c = "repair_missing_liveness" -> TRUE
    [] Bug = "repair_allows_wrong_highest_parent"
       /\ c = "repair_highest_not_parent" -> TRUE
    [] Bug = "repair_allows_noncanonical_highest"
       /\ c = "repair_highest_not_canonical" -> TRUE
    [] Bug = "repair_ignores_consensus_lock"
       /\ c = "repair_consensus_lock" -> TRUE
    [] Bug = "repair_ignores_observed_qc"
       /\ c = "repair_observed_qc" -> TRUE
    [] Bug = "repair_ignores_any_recoverable_qc"
       /\ c = "repair_any_recoverable_qc" -> TRUE
    [] Bug = "repair_ignores_vote_lock"
       /\ c = "repair_vote_lock" -> TRUE
    [] Bug = "repair_ignores_inflight"
       /\ c = "repair_inflight_valid" -> TRUE
    [] OTHER -> SpecRepairAllows(c)

SpecRepairOutput(c) ==
  BoolToInt(SpecRepairAllows(c))

ActualRepairOutput(c) ==
  BoolToInt(ActualRepairAllows(c))

AssemblyBaseBlocks(c) ==
  c # "assembly_base_allows"

AssemblyRepairAllows(c) ==
  c \in {"assembly_repair_parent", "assembly_repair_retired"}

SpecAssemblyBlocks(c) ==
  AssemblyBaseBlocks(c) /\ ~AssemblyRepairAllows(c)

ActualAssemblyBaseBlocks(c) ==
  CASE Bug = "assembly_ignores_base"
       /\ c = "assembly_base_blocks_no_repair" -> FALSE
    [] OTHER -> AssemblyBaseBlocks(c)

ActualAssemblyRepairAllows(c) ==
  CASE Bug = "assembly_ignores_repair"
       /\ c = "assembly_repair_parent" -> FALSE
    [] Bug = "assembly_allows_fresh_pending_repair"
       /\ c = "assembly_repair_fresh_pending" -> TRUE
    [] OTHER -> AssemblyRepairAllows(c)

ActualAssemblyBlocks(c) ==
  ActualAssemblyBaseBlocks(c) /\ ~ActualAssemblyRepairAllows(c)

SpecAssemblyOutput(c) ==
  BoolToInt(SpecAssemblyBlocks(c))

ActualAssemblyOutput(c) ==
  BoolToInt(ActualAssemblyBlocks(c))

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "base_allows_future_view",
       "base_allows_commit_conflict",
       "base_ignores_same_view_hard_lock",
       "base_ignores_same_view_live_material",
       "base_blocks_same_view_clear",
       "base_resilience_disabled_allows_old_vote",
       "base_ignores_old_vote_hard_lock",
       "base_ignores_fresh_pending",
       "base_active_owner_uses_short_window",
       "base_stale_pending_still_blocks",
       "base_recovery_age_exhaustion_ignored",
       "base_view_gap_exhaustion_ignored",
       "base_ignores_frontier_commit_qc",
       "base_ignores_frontier_competing_lock",
       "base_frontier_no_progress_blocks",
       "repair_rejects_parent_marker",
       "repair_rejects_retired_pending",
       "repair_rejects_retry_aborted_pending",
       "repair_rejects_invalid_pending",
       "repair_rejects_stale_no_qc_pending",
       "repair_rejects_absent_pending",
       "repair_allows_fresh_pending",
       "repair_allows_commit_qc_pending",
       "repair_ignores_resilience",
       "repair_allows_prepare_phase",
       "repair_allows_equal_view",
       "repair_allows_nonfrontier_height",
       "repair_allows_missing_liveness",
       "repair_allows_wrong_highest_parent",
       "repair_allows_noncanonical_highest",
       "repair_ignores_consensus_lock",
       "repair_ignores_observed_qc",
       "repair_ignores_any_recoverable_qc",
       "repair_ignores_vote_lock",
       "repair_ignores_inflight",
       "assembly_ignores_base",
       "assembly_ignores_repair",
       "assembly_allows_fresh_pending_repair"
     }
  /\ checked = 0

SafetyFast ==
  /\ \A c \in BaseCases: ActualBaseOutput(c) = SpecBaseOutput(c)
  /\ \A c \in RepairCases: ActualRepairOutput(c) = SpecRepairOutput(c)
  /\ \A c \in AssemblyCases:
       ActualAssemblyOutput(c) = SpecAssemblyOutput(c)

BugBaseAllowsFutureView ==
  ActualBaseOutput("future_view_blocks") = SpecBaseOutput("future_view_blocks")

BugBaseAllowsCommitConflict ==
  ActualBaseOutput("commit_non_parent_blocks") =
    SpecBaseOutput("commit_non_parent_blocks")

BugBaseIgnoresSameViewHardLock ==
  ActualBaseOutput("same_view_hard_lock") =
    SpecBaseOutput("same_view_hard_lock")

BugBaseIgnoresSameViewLiveMaterial ==
  ActualBaseOutput("same_view_live_material") =
    SpecBaseOutput("same_view_live_material")

BugBaseBlocksSameViewClear ==
  ActualBaseOutput("same_view_clear") = SpecBaseOutput("same_view_clear")

BugBaseResilienceDisabledAllowsOldVote ==
  ActualBaseOutput("resilience_disabled_old_vote") =
    SpecBaseOutput("resilience_disabled_old_vote")

BugBaseIgnoresOldVoteHardLock ==
  ActualBaseOutput("old_vote_hard_lock") =
    SpecBaseOutput("old_vote_hard_lock")

BugBaseIgnoresFreshPending ==
  ActualBaseOutput("old_vote_fresh_pending") =
    SpecBaseOutput("old_vote_fresh_pending")

BugBaseActiveOwnerUsesShortWindow ==
  ActualBaseOutput("old_vote_active_owner_mid_age") =
    SpecBaseOutput("old_vote_active_owner_mid_age")

BugBaseStalePendingStillBlocks ==
  ActualBaseOutput("old_vote_pending_stale") =
    SpecBaseOutput("old_vote_pending_stale")

BugBaseRecoveryAgeExhaustionIgnored ==
  ActualBaseOutput("old_vote_recovery_age_exhausted") =
    SpecBaseOutput("old_vote_recovery_age_exhausted")

BugBaseViewGapExhaustionIgnored ==
  ActualBaseOutput("old_vote_view_gap_exhausted") =
    SpecBaseOutput("old_vote_view_gap_exhausted")

BugBaseIgnoresFrontierCommitQc ==
  ActualBaseOutput("frontier_commit_qc_blocks") =
    SpecBaseOutput("frontier_commit_qc_blocks")

BugBaseIgnoresFrontierCompetingLock ==
  ActualBaseOutput("frontier_competing_lock_blocks") =
    SpecBaseOutput("frontier_competing_lock_blocks")

BugBaseFrontierNoProgressBlocks ==
  ActualBaseOutput("frontier_no_progress") =
    SpecBaseOutput("frontier_no_progress")

BugRepairRejectsParentMarker ==
  ActualRepairOutput("repair_parent_marker") =
    SpecRepairOutput("repair_parent_marker")

BugRepairRejectsRetiredPending ==
  ActualRepairOutput("repair_retired_pending") =
    SpecRepairOutput("repair_retired_pending")

BugRepairRejectsRetryAbortedPending ==
  ActualRepairOutput("repair_retry_aborted_pending") =
    SpecRepairOutput("repair_retry_aborted_pending")

BugRepairRejectsInvalidPending ==
  ActualRepairOutput("repair_invalid_pending") =
    SpecRepairOutput("repair_invalid_pending")

BugRepairRejectsStaleNoQcPending ==
  ActualRepairOutput("repair_stale_no_qc_pending") =
    SpecRepairOutput("repair_stale_no_qc_pending")

BugRepairRejectsAbsentPending ==
  ActualRepairOutput("repair_absent_pending") =
    SpecRepairOutput("repair_absent_pending")

BugRepairAllowsFreshPending ==
  ActualRepairOutput("repair_fresh_pending") =
    SpecRepairOutput("repair_fresh_pending")

BugRepairAllowsCommitQcPending ==
  ActualRepairOutput("repair_commit_qc_pending") =
    SpecRepairOutput("repair_commit_qc_pending")

BugRepairIgnoresResilience ==
  ActualRepairOutput("repair_resilience_disabled") =
    SpecRepairOutput("repair_resilience_disabled")

BugRepairAllowsPreparePhase ==
  ActualRepairOutput("repair_prepare_phase") =
    SpecRepairOutput("repair_prepare_phase")

BugRepairAllowsEqualView ==
  ActualRepairOutput("repair_equal_view") =
    SpecRepairOutput("repair_equal_view")

BugRepairAllowsNonfrontierHeight ==
  ActualRepairOutput("repair_nonfrontier_height") =
    SpecRepairOutput("repair_nonfrontier_height")

BugRepairAllowsMissingLiveness ==
  ActualRepairOutput("repair_missing_liveness") =
    SpecRepairOutput("repair_missing_liveness")

BugRepairAllowsWrongHighestParent ==
  ActualRepairOutput("repair_highest_not_parent") =
    SpecRepairOutput("repair_highest_not_parent")

BugRepairAllowsNoncanonicalHighest ==
  ActualRepairOutput("repair_highest_not_canonical") =
    SpecRepairOutput("repair_highest_not_canonical")

BugRepairIgnoresConsensusLock ==
  ActualRepairOutput("repair_consensus_lock") =
    SpecRepairOutput("repair_consensus_lock")

BugRepairIgnoresObservedQc ==
  ActualRepairOutput("repair_observed_qc") =
    SpecRepairOutput("repair_observed_qc")

BugRepairIgnoresAnyRecoverableQc ==
  ActualRepairOutput("repair_any_recoverable_qc") =
    SpecRepairOutput("repair_any_recoverable_qc")

BugRepairIgnoresVoteLock ==
  ActualRepairOutput("repair_vote_lock") =
    SpecRepairOutput("repair_vote_lock")

BugRepairIgnoresInflight ==
  ActualRepairOutput("repair_inflight_valid") =
    SpecRepairOutput("repair_inflight_valid")

BugAssemblyIgnoresBase ==
  ActualAssemblyOutput("assembly_base_blocks_no_repair") =
    SpecAssemblyOutput("assembly_base_blocks_no_repair")

BugAssemblyIgnoresRepair ==
  ActualAssemblyOutput("assembly_repair_parent") =
    SpecAssemblyOutput("assembly_repair_parent")

BugAssemblyAllowsFreshPendingRepair ==
  ActualAssemblyOutput("assembly_repair_fresh_pending") =
    SpecAssemblyOutput("assembly_repair_fresh_pending")

====
