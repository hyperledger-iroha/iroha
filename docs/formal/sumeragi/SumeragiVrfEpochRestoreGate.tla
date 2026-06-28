---- MODULE SumeragiVrfEpochRestoreGate ----
EXTENDS FiniteSets

(***************************************************************************
A bounded abstract model for NPoS VRF epoch restore and observation hydration.

This slice covers `EpochManager::restore_from_record(...)`,
`snapshot_current_epoch(...)`, and `merge_record_observations(...)` from
`crates/iroha_core/src/sumeragi/epoch.rs`. It abstracts cryptographic entropy
as labels while preserving the deterministic state contract:
- unfinalized records restore epoch, seed, params, participant commits/reveals,
  late reveals, contiguous roster snapshots, and clear transient reports,
- zero epoch length is floored to one and window offsets are clamped to length,
- finalized records derive the next epoch seed, advance the epoch with
  saturating arithmetic, and clear per-epoch inputs and roster,
- snapshots prefer the live roster length over the hint, preserve current
  inputs and update height, and never synthesize finalized penalty lists, and
- record-observation merge ignores wrong epochs, fills only absent observations,
  preserves existing observations, hydrates late reveals, and leaves identity
  state untouched.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Str;
  candidate,
  \* @type: Set(Str);
  fields

\* @type: <<Str, Set(Str)>>;
vars == <<candidate, fields>>

Cases == {
  "restore_unfinalized_inputs",
  "restore_zero_roster_none",
  "restore_params_zero_length",
  "restore_params_offsets_clamped",
  "restore_finalized_advances",
  "restore_finalized_epoch_saturates",
  "restore_clears_last_reports",
  "snapshot_uses_roster",
  "snapshot_uses_hint_without_roster",
  "snapshot_preserves_inputs",
  "snapshot_empty_penalty_lists",
  "merge_wrong_epoch_ignored",
  "merge_missing_observations",
  "merge_preserves_existing_observations",
  "merge_adds_late_reveal",
  "merge_preserves_existing_late_reveal",
  "merge_preserves_state_identity"
}

AllFields == {
  "epoch_record",
  "seed_record",
  "commits_record",
  "reveals_record",
  "late_reveals_record",
  "roster_contiguous",
  "roster_none",
  "reports_cleared",
  "not_finalized_active",
  "length_min_one",
  "commit_clamped_to_length",
  "reveal_clamped_to_length",
  "epoch_incremented",
  "next_seed_from_record",
  "inputs_cleared",
  "finalized_active_cleared",
  "epoch_saturates",
  "snapshot_epoch",
  "snapshot_seed",
  "roster_len_from_roster",
  "roster_len_from_hint",
  "updated_height",
  "penalty_lists_empty",
  "commits_snapshot",
  "reveals_snapshot",
  "late_reveals_snapshot",
  "seed_snapshot",
  "epoch_snapshot",
  "committed_no_reveal_empty",
  "no_participation_empty",
  "no_commit_added",
  "no_reveal_added",
  "no_late_added",
  "state_identity_preserved",
  "commit_added",
  "reveal_added",
  "existing_preserved",
  "commit_existing_kept",
  "reveal_existing_kept",
  "incoming_conflict_ignored",
  "late_reveal_added",
  "late_reveal_existing_kept",
  "epoch_preserved",
  "seed_preserved",
  "params_preserved",
  "roster_preserved",
  "reports_preserved"
}

SpecFields(c) ==
  CASE c = "restore_unfinalized_inputs" ->
      {"epoch_record", "seed_record", "commits_record", "reveals_record",
       "late_reveals_record", "roster_contiguous", "reports_cleared",
       "not_finalized_active"}
    [] c = "restore_zero_roster_none" ->
      {"roster_none", "reports_cleared"}
    [] c = "restore_params_zero_length" ->
      {"length_min_one", "commit_clamped_to_length",
       "reveal_clamped_to_length"}
    [] c = "restore_params_offsets_clamped" ->
      {"commit_clamped_to_length", "reveal_clamped_to_length"}
    [] c = "restore_finalized_advances" ->
      {"epoch_incremented", "next_seed_from_record", "inputs_cleared",
       "roster_none", "reports_cleared", "finalized_active_cleared"}
    [] c = "restore_finalized_epoch_saturates" ->
      {"epoch_saturates", "inputs_cleared", "roster_none"}
    [] c = "restore_clears_last_reports" ->
      {"reports_cleared"}
    [] c = "snapshot_uses_roster" ->
      {"snapshot_epoch", "snapshot_seed", "roster_len_from_roster",
       "updated_height", "penalty_lists_empty"}
    [] c = "snapshot_uses_hint_without_roster" ->
      {"roster_len_from_hint", "penalty_lists_empty"}
    [] c = "snapshot_preserves_inputs" ->
      {"commits_snapshot", "reveals_snapshot", "late_reveals_snapshot",
       "seed_snapshot", "epoch_snapshot"}
    [] c = "snapshot_empty_penalty_lists" ->
      {"committed_no_reveal_empty", "no_participation_empty"}
    [] c = "merge_wrong_epoch_ignored" ->
      {"no_commit_added", "no_reveal_added", "no_late_added",
       "state_identity_preserved"}
    [] c = "merge_missing_observations" ->
      {"commit_added", "reveal_added", "existing_preserved",
       "state_identity_preserved"}
    [] c = "merge_preserves_existing_observations" ->
      {"commit_existing_kept", "reveal_existing_kept",
       "incoming_conflict_ignored", "state_identity_preserved"}
    [] c = "merge_adds_late_reveal" ->
      {"late_reveal_added", "state_identity_preserved"}
    [] c = "merge_preserves_existing_late_reveal" ->
      {"late_reveal_existing_kept", "incoming_conflict_ignored",
       "state_identity_preserved"}
    [] c = "merge_preserves_state_identity" ->
      {"epoch_preserved", "seed_preserved", "params_preserved",
       "roster_preserved", "reports_preserved"}
    [] OTHER -> {}

ActualFields(c) ==
  CASE Bug = "restore_drops_commit" /\ c = "restore_unfinalized_inputs" ->
      SpecFields(c) \ {"commits_record"}
    [] Bug = "restore_drops_reveal" /\ c = "restore_unfinalized_inputs" ->
      SpecFields(c) \ {"reveals_record"}
    [] Bug = "restore_drops_late_reveal" /\ c = "restore_unfinalized_inputs" ->
      SpecFields(c) \ {"late_reveals_record"}
    [] Bug = "restore_builds_sparse_roster" /\ c = "restore_unfinalized_inputs" ->
      SpecFields(c) \ {"roster_contiguous"}
    [] Bug = "restore_keeps_zero_length" /\ c = "restore_params_zero_length" ->
      SpecFields(c) \ {"length_min_one"}
    [] Bug = "restore_skips_offset_clamp"
       /\ c \in {"restore_params_zero_length", "restore_params_offsets_clamped"} ->
      SpecFields(c) \ {"commit_clamped_to_length", "reveal_clamped_to_length"}
    [] Bug = "restore_finalized_keeps_inputs" /\ c = "restore_finalized_advances" ->
      SpecFields(c) \ {"inputs_cleared"}
    [] Bug = "restore_finalized_does_not_advance" /\ c = "restore_finalized_advances" ->
      SpecFields(c) \ {"epoch_incremented"}
    [] Bug = "restore_finalized_wraps_epoch" /\ c = "restore_finalized_epoch_saturates" ->
      SpecFields(c) \ {"epoch_saturates"}
    [] Bug = "restore_keeps_last_reports"
       /\ c \in {"restore_unfinalized_inputs", "restore_zero_roster_none",
                 "restore_finalized_advances", "restore_clears_last_reports"} ->
      SpecFields(c) \ {"reports_cleared"}
    [] Bug = "snapshot_uses_hint_over_roster" /\ c = "snapshot_uses_roster" ->
      (SpecFields(c) \ {"roster_len_from_roster"}) \union {"roster_len_from_hint"}
    [] Bug = "snapshot_drops_late_reveals" /\ c = "snapshot_preserves_inputs" ->
      SpecFields(c) \ {"late_reveals_snapshot"}
    [] Bug = "snapshot_carries_penalty_lists" /\ c = "snapshot_empty_penalty_lists" ->
      SpecFields(c) \ {"committed_no_reveal_empty", "no_participation_empty"}
    [] Bug = "snapshot_drops_updated_height" /\ c = "snapshot_uses_roster" ->
      SpecFields(c) \ {"updated_height"}
    [] Bug = "merge_wrong_epoch_mutates" /\ c = "merge_wrong_epoch_ignored" ->
      (SpecFields(c) \ {"no_commit_added", "no_reveal_added", "no_late_added"})
        \union {"commit_added", "reveal_added", "late_reveal_added"}
    [] Bug = "merge_overwrites_existing_commit"
       /\ c = "merge_preserves_existing_observations" ->
      SpecFields(c) \ {"commit_existing_kept"}
    [] Bug = "merge_overwrites_existing_reveal"
       /\ c = "merge_preserves_existing_observations" ->
      SpecFields(c) \ {"reveal_existing_kept"}
    [] Bug = "merge_skips_missing_commit" /\ c = "merge_missing_observations" ->
      SpecFields(c) \ {"commit_added"}
    [] Bug = "merge_skips_missing_reveal" /\ c = "merge_missing_observations" ->
      SpecFields(c) \ {"reveal_added"}
    [] Bug = "merge_skips_late_reveal" /\ c = "merge_adds_late_reveal" ->
      SpecFields(c) \ {"late_reveal_added"}
    [] Bug = "merge_overwrites_late_reveal"
       /\ c = "merge_preserves_existing_late_reveal" ->
      SpecFields(c) \ {"late_reveal_existing_kept"}
    [] Bug = "merge_mutates_identity" /\ c = "merge_preserves_state_identity" ->
      SpecFields(c) \ {"epoch_preserved", "seed_preserved",
                       "params_preserved", "roster_preserved"}
    [] OTHER -> SpecFields(c)

Init ==
  /\ candidate = "none"
  /\ fields = {}

Apply(c) ==
  /\ candidate' = c
  /\ fields' = ActualFields(c)

Stable ==
  UNCHANGED vars

Next ==
  \/ \E c \in Cases: Apply(c)
  \/ Stable

TypeInvariant ==
  /\ Bug \in {
       "none",
       "restore_drops_commit",
       "restore_drops_reveal",
       "restore_drops_late_reveal",
       "restore_builds_sparse_roster",
       "restore_keeps_zero_length",
       "restore_skips_offset_clamp",
       "restore_finalized_keeps_inputs",
       "restore_finalized_does_not_advance",
       "restore_finalized_wraps_epoch",
       "restore_keeps_last_reports",
       "snapshot_uses_hint_over_roster",
       "snapshot_drops_late_reveals",
       "snapshot_carries_penalty_lists",
       "snapshot_drops_updated_height",
       "merge_wrong_epoch_mutates",
       "merge_overwrites_existing_commit",
       "merge_overwrites_existing_reveal",
       "merge_skips_missing_commit",
       "merge_skips_missing_reveal",
       "merge_skips_late_reveal",
       "merge_overwrites_late_reveal",
       "merge_mutates_identity"
     }
  /\ candidate \in Cases \cup {"none"}
  /\ fields \subseteq AllFields

FieldsMatchSpec ==
  candidate = "none" \/ fields = SpecFields(candidate)

VrfEpochRestoreExactness ==
  /\ FieldsMatchSpec

VrfEpochRestoreCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ VrfEpochRestoreExactness

NoBugInvariant == VrfEpochRestoreExactness

Safety ==
  VrfEpochRestoreExactness

====
